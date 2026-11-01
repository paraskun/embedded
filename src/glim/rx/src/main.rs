#![no_main]
#![no_std]

extern crate alloc;

mod ddp;
mod cap;
mod nvm;

use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use core::sync::atomic::{AtomicU16, Ordering};

use edge_dhcp::io::{self, DEFAULT_SERVER_PORT};
use edge_dhcp::server::{Server, ServerOptions};
use edge_http::io::server::Server as HttpServer;
use edge_nal::{TcpBind, UdpBind};
use edge_nal_embassy::{Tcp, TcpBuffers, Udp, UdpBuffers};

use embassy_executor::Spawner;
use embassy_futures::select;
use embassy_net::{Ipv4Cidr, Stack, StackResources};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_time::Timer;

use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::i2c;
use esp_hal::{Async, i2c::master::I2c, time::Rate};

use esp_radio::wifi::Config::AccessPointStation;
use esp_radio::wifi::ControllerConfig;
use esp_radio::wifi::ap::AccessPointConfig;
use esp_radio::wifi::sta::StationConfig;
use static_cell::StaticCell;

static DA: u8 = 0x42;
static ST_SR: StaticCell<StackResources<3>> = StaticCell::new();
static AP_SR: StaticCell<StackResources<3>> = StaticCell::new();
static CHAN: Channel<CriticalSectionRawMutex, cap::Credentials, 1> = Channel::new();
pub static PIXELS: AtomicU16 = AtomicU16::new(0);
static PAGE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index.html.gz"));

esp_bootloader_esp_idf::esp_app_desc!();

fn ap_conf() -> AccessPointConfig {
    esp_radio::wifi::ap::AccessPointConfig::default()
        .with_ssid("Glim".try_into().unwrap())
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);

    let hal = esp_hal::init(esp_hal::Config::default());
    let tg = esp_hal::timer::timg::TimerGroup::new(hal.TIMG0);
    esp_rtos::start(tg.timer0, hal.FROM_CPU_INTR0);

    let bus = i2c::master::I2c::new(
        hal.I2C0,
        i2c::master::Config::default()
            .with_frequency(Rate::from_khz(100)))
        .unwrap()
        .with_scl(hal.GPIO9)
        .with_sda(hal.GPIO8)
        .into_async();

    let mut flash = esp_storage::FlashStorage::new(hal.FLASH);
    let cred = nvm::load(&mut flash).unwrap_or_else(cap::Credentials::default);

    let wifi_cfg = esp_radio::wifi::Config::AccessPointStation(
        StationConfig::default(), ap_conf());
    let wifi_ctl = esp_radio::wifi::WifiController::new(
        hal.WIFI,
        ControllerConfig::default().with_initial_config(wifi_cfg)).unwrap();

    let wifi_st = esp_radio::wifi::Interface::station();
    let wifi_ap = esp_radio::wifi::Interface::access_point();

    let rng = esp_hal::rng::Rng::new();
    let gway = Ipv4Addr::new(192, 168, 0, 1);

    let (st_stack, st_runner) = embassy_net::new(
        wifi_st,
        embassy_net::Config::dhcpv4(Default::default()),
        ST_SR.init(StackResources::new()),
        (rng.random() as u64) << 32 | (rng.random() as u64));

    let (ap_stack, ap_runner) = embassy_net::new(
        wifi_ap,
        embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
            address: Ipv4Cidr::new(gway, 24),
            gateway: Some(gway),
            dns_servers: Default::default(),
        }),
        AP_SR.init(StackResources::new()),
        (rng.random() as u64) << 32 | (rng.random() as u64));

    spawner.spawn(task_net(st_runner).unwrap());
    spawner.spawn(task_net(ap_runner).unwrap());
    spawner.spawn(task_mmt(wifi_ctl, CHAN.receiver(), cred, flash).unwrap());

    ap_stack.wait_config_up().await;

    spawner.spawn(task_dhcp(ap_stack, gway).unwrap());
    spawner.spawn(task_cap(ap_stack, st_stack, CHAN.sender()).unwrap());
    spawner.spawn(task_ddp(st_stack, bus).unwrap());

    loop {
        st_stack.wait_config_up().await;

        if let Some(config) = st_stack.config_v4() {
            esp_println::println!("sta ip: {}", config.address);
        }

        st_stack.wait_config_down().await;
    }
}

#[embassy_executor::task(pool_size = 2)]
async fn task_net(mut runner: embassy_net::Runner<'static, esp_radio::wifi::Interface>) {
    runner.run().await
}

#[embassy_executor::task]
async fn task_mmt(
    mut ctl: esp_radio::wifi::WifiController<'static>,
    rcv: Receiver<'static, CriticalSectionRawMutex, cap::Credentials, 1>,
    mut cred: cap::Credentials,
    mut flash: esp_storage::FlashStorage<'static>,
) {
    let mut apply = true;
    let mut save = false;
    let mut connected = false;

    loop {
        if apply {
            apply = false;
            connected = false;

            let _ = ctl.disconnect_async().await;

            if save {
                save = false;
                nvm::store(&mut flash, &cred);
            }

            let st_conf = StationConfig::default()
                .with_ssid(cred.ssid().try_into().unwrap())
                .with_authentication(esp_radio::wifi::AuthenticationMethodConfig::WpaWpa2Personal(
                        cred.pass().try_into().unwrap()));

            if ctl.set_config(&AccessPointStation(st_conf, ap_conf())).is_ok() {
                connected = ctl.connect_async().await.is_ok();
                esp_println::println!("sta: {}", connected);
            }
        }

        if connected {
            match select::select(ctl.wait_for_disconnect_async(), rcv.receive()).await {
                select::Either::First(_) => {
                    connected = false;
                    Timer::after_secs(2).await;
                    apply = true;
                }
                select::Either::Second(c) => { cred = c; apply = true; save = true; }
            }
        } else {
            let c = rcv.receive().await;

            cred = c;
            apply = true;
            save = true;
        }
    }
}

#[embassy_executor::task]
async fn task_dhcp(stack: Stack<'static>, ip: Ipv4Addr) {
    let udp_buf = UdpBuffers::<1, 1024, 1024, 10>::new();
    let udp = Udp::new(stack, &udp_buf);

    let mut sock = udp.bind(SocketAddr::V4(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        DEFAULT_SERVER_PORT
    ))).await.unwrap();

    let mut gw_buf = [Ipv4Addr::UNSPECIFIED];
    let mut buf = [0u8; 1500];

    let opts = ServerOptions::new(ip, Some(&mut gw_buf));
    let mut srv = Server::<_, 64>::new(
        || embassy_time::Instant::now().as_secs(),
        ip
    );

    loop {
        if let Err(e) = io::server::run(&mut srv, &opts, &mut sock, &mut buf).await {
            esp_println::println!("dhcp error: {:?}", e)
        }

        embassy_time::Timer::after_millis(500).await;
    }
}

#[embassy_executor::task]
async fn task_cap(
    stack: Stack<'static>,
    sta: Stack<'static>,
    snd: Sender<'static, CriticalSectionRawMutex, cap::Credentials, 1>,
) {
    let tcp_buf = TcpBuffers::<1, 1024, 1024>::new();
    let tcp = Tcp::new(stack, &tcp_buf);

    let acceptor = tcp.bind(SocketAddr::V4(SocketAddrV4::new(
        Ipv4Addr::UNSPECIFIED,
        80,
    ))).await.unwrap();

    let mut srv = HttpServer::<1, 1024, 16>::new();

    loop {
        let acceptor = edge_nal::WithTimeout::new(3000, acceptor);
        let handler = cap::CaptureHandler { snd, sta };

        if let Err(e) = srv.run(Some(3000), acceptor, handler).await {
            esp_println::println!("http error: {:?}", e);
            Timer::after_millis(500).await;
        }
    }
}

async fn read_pixels(i2c: &mut I2c<'static, Async>) -> Option<u16> {
    let mut size = [0; 2];

    match i2c.read_async(DA, &mut size).await {
        Ok(()) => {
            let pixels = u16::from_be_bytes(size);

            if (1..=ddp::MAX_PIXELS).contains(&pixels) {
                return Some(pixels);
            }

            esp_println::println!("invalid tx pixel count: {}", pixels);
        }
        Err(e) => esp_println::println!("tx pixel count error: {:?}", e),
    }

    None
}

#[embassy_executor::task]
async fn task_ddp(stack: Stack<'static>, mut i2c: I2c<'static, Async>) {
    let mut pixels = loop {
        if let Some(pixels) = read_pixels(&mut i2c).await {
            break pixels;
        }

        Timer::after_millis(500).await;
    };

    esp_println::println!("tx pixels: {}", pixels);
    PIXELS.store(pixels, Ordering::Relaxed);

    let mut srv = ddp::Listener::new(stack);

    loop {
        if let Some(frame) = srv.recv(pixels).await {
            if let Err(e) = i2c.write_async(DA, frame).await {
                esp_println::println!("tx frame error: {:?}", e);
                continue;
            }

            if let Some(new) = read_pixels(&mut i2c).await
                && new != pixels
            {
                esp_println::println!("tx pixels: {}", new);
                pixels = new;
                PIXELS.store(new, Ordering::Relaxed);
            }
        }
    }
}
