#![no_main]
#![no_std]

mod ddp;
mod cap;

use core::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

use edge_dhcp::io::{self, DEFAULT_SERVER_PORT};
use edge_dhcp::server::{Server, ServerOptions};
use edge_nal_embassy::{Udp, UdpBuffers};
use edge_nal::UdpBind;

use embassy_executor::Spawner;
use embassy_futures::select;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Ipv4Cidr, Stack, StackResources};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_time::{Duration, Timer};

use esp_alloc as _;
use esp_backtrace as _;
use esp_hal::i2c;
use esp_hal::{Async, i2c::master::I2c, time::Rate};

use embedded_io_async::Write;
use esp_radio::wifi::Config::AccessPointStation;
use esp_radio::wifi::ControllerConfig;
use esp_radio::wifi::ap::AccessPointConfig;
use esp_radio::wifi::sta::StationConfig;
use static_cell::StaticCell;

static DA: u8 = 0x42;
static ST_SR: StaticCell<StackResources<3>> = StaticCell::new();
static AP_SR: StaticCell<StackResources<3>> = StaticCell::new();
static CHAN: Channel<CriticalSectionRawMutex, cap::Credentials, 1> = Channel::new();
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

    let wifi_cfg = esp_radio::wifi::Config::AccessPointStation(
        StationConfig::default(), ap_conf());
    let wifi_ctl = esp_radio::wifi::WifiController::new(
        hal.WIFI,
        ControllerConfig::default().with_initial_config(wifi_cfg)).unwrap();

    let wifi_st = esp_radio::wifi::Interface::station();
    let wifi_ap = esp_radio::wifi::Interface::access_point();

    let rng = esp_hal::rng::Rng::new();
    let seed = (rng.random() as u64) << 32 | (rng.random() as u64);
    let gway = Ipv4Addr::new(192, 168, 0, 1);

    let (st_stack, st_runner) = embassy_net::new(
        wifi_st,
        embassy_net::Config::dhcpv4(Default::default()),
        ST_SR.init(StackResources::new()),
        seed);

    let (ap_stack, ap_runner) = embassy_net::new(
        wifi_ap,
        embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
            address: Ipv4Cidr::new(gway, 24),
            gateway: Some(gway),
            dns_servers: Default::default(),
        }),
        AP_SR.init(StackResources::new()),
        seed);

    spawner.spawn(task_net(st_runner).unwrap());
    spawner.spawn(task_net(ap_runner).unwrap());
    spawner.spawn(task_mmt(wifi_ctl, CHAN.receiver()).unwrap());

    ap_stack.wait_config_up().await;

    spawner.spawn(task_dhcp(ap_stack, gway).unwrap());
    spawner.spawn(task_cap(ap_stack, CHAN.sender()).unwrap());
    spawner.spawn(task_ddp(st_stack, bus).unwrap());

    loop {
        st_stack.wait_config_up().await;
        ap_stack.wait_config_up().await;

        if let Some(config) = st_stack.config_v4() {
            esp_println::println!("Got IP: {}", config.address);
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
) {
    let mut cred = cap::default();
    let mut apply = true;
    let mut connected = false;

    loop {
        if apply {
            apply = false;
            connected = false;

            let _ = ctl.disconnect_async().await;

            let st_conf = StationConfig::default()
                .with_ssid(cred.ssid().try_into().unwrap())
                .with_authentication(esp_radio::wifi::AuthenticationMethodConfig::WpaWpa2Personal(
                        cred.pass().try_into().unwrap()));

            if ctl.set_config(&AccessPointStation(st_conf, ap_conf())).is_ok() {
                connected = ctl.connect_async().await.is_ok();
                esp_println::println!("STA connect: {}", connected);
            }
        }

        if connected {
            match select::select(ctl.wait_for_disconnect_async(), rcv.receive()).await {
                select::Either::First(_) => {
                    connected = false;
                    Timer::after_secs(2).await;
                    apply = true;
                }
                select::Either::Second(c) => { cred = c; apply = true; }
            }
        } else {
            let c = rcv.receive().await;

            cred = c;
            apply = true;
        }

        Timer::after_millis(1000).await;
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
    snd: Sender<'static, CriticalSectionRawMutex, cap::Credentials, 1>,
) {
    let mut rx = [0u8; 1024];
    let mut tx = [0u8; 1024];

    loop {
        let mut sock = TcpSocket::new(stack, &mut rx, &mut tx);
        sock.set_timeout(Some(Duration::from_secs(3)));

        if let Err(_) = sock.accept(80).await {
            continue;
        }

        let mut buf = [0u8; 1024];
        let mut len = 0;

        loop {
            match sock.read(&mut buf[len..]).await {
                Ok(0) => break,
                Ok(n) => {
                    len += n;

                    if buf[..len].windows(4).any(|w| w == b"\r\n\r\n") || len == buf.len() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        let req = core::str::from_utf8(&buf[..len]).unwrap_or("");
        let path = req.split_whitespace().nth(1).unwrap_or("/");

        match path {
            "/" => {
                let mut hdr = [0u8; 128];
                let n = fmt_200(&mut hdr, PAGE.len());
                sock.write_all(&hdr[..n]).await.unwrap();
                sock.write_all(PAGE).await.unwrap();
            }
            _ => {
                if path.starts_with("/set") {
                    let mut tok = path.split("/");

                    let _ = tok.next();
                    let _ = tok.next();
                    let ssid = tok.next().unwrap();
                    let pass = tok.next().unwrap();

                    sock.write_all("
                        HTTP/1.1 204 No Content\r\n\
                        Connection: close\r\n\
                        \r\n".as_bytes()).await.unwrap();

                    esp_println::println!("Received credentials: {} / {}",
                        ssid, pass);

                    snd.send(cap::Credentials::new(ssid, pass)).await;
                }
            }
        }

        let _ = sock.flush().await;
        sock.close();
    }
}

fn fmt_200(buf: &mut [u8], content_len: usize) -> usize {
    use core::fmt::Write;
    let mut w = BufWriter { buf, pos: 0 };
    write!(w, "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Encoding: gzip\r\nContent-Length: {content_len}\r\nConnection: close\r\n\r\n").unwrap();
    w.pos
}

struct BufWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl core::fmt::Write for BufWriter<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        self.buf[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
        Ok(())
    }
}

#[embassy_executor::task]
async fn task_ddp(stack: Stack<'static>, mut i2c: I2c<'static, Async>) {
    let mut srv = ddp::Listener::new(stack);
    let mut pkt = ddp::Packet::default();

    loop {
        srv.recv(&mut pkt).await;

        let body: &[u8] = unsafe { bytemuck::cast_slice(&pkt.body.rgb) };
        let tx = &body[..pkt.header.size as usize];

        i2c.write_async(DA, tx).await.unwrap();
    }
}
