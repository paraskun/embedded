#![no_main]
#![no_std]

mod ddp;
mod cap;

use embassy_executor::Spawner;
use embassy_net::{Stack, StackResources};
use embassy_time::Timer;

use esp_alloc as _;
use esp_hal::i2c;

use esp_hal::{Async, i2c::master::I2c, time::Rate};
use panic_reset as _;
use static_cell::StaticCell;

static DA: u8 = 0x42;
static SR: StaticCell<StackResources<3>> = StaticCell::new();

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 64 * 1024);

    let hal = esp_hal::init(esp_hal::Config::default());
    let tg = esp_hal::timer::timg::TimerGroup::new(hal.TIMG0);
    esp_rtos::start(tg.timer0, hal.FROM_CPU_INTR0);

    // let bus = Spi::new(
    //     hal.SPI2,
    //     Config::default()
    //         .with_frequency(Rate::from_mhz(1))
    //         .with_mode(Mode::_0))
    //     .unwrap()
    //     .with_sck(hal.GPIO6)
    //     .with_mosi(hal.GPIO7)
    //     .with_miso(hal.GPIO2)
    //     .with_cs(hal.GPIO10)
    //     .with_dma(hal.DMA_CH0)
    //     .with_buffers(
    //         esp_hal::dma_rx_buffer!(4096).unwrap(),
    //         esp_hal::dma_tx_buffer!(4096).unwrap())
    //     .into_async();

    let bus = i2c::master::I2c::new(
        hal.I2C0,
        i2c::master::Config::default()
            .with_frequency(Rate::from_khz(100)))
        .unwrap()
        .with_scl(hal.GPIO9)
        .with_sda(hal.GPIO8)
        .into_async();

    let wifi_ife = esp_radio::wifi::Interface::station();
    let wifi_ctl = esp_radio::wifi::WifiController::new(
        hal.WIFI,
        Default::default()).unwrap();

    let rng = esp_hal::rng::Rng::new();
    let seed = (rng.random() as u64) << 32 | (rng.random() as u64);

    let (stack, runner) = embassy_net::new(
        wifi_ife,
        embassy_net::Config::dhcpv4(Default::default()),
        SR.init(StackResources::new()),
        seed);

    spawner.spawn(task_net(runner).unwrap());
    spawner.spawn(task_mmt(wifi_ctl).unwrap());
    spawner.spawn(task_ddp(stack, bus).unwrap());

    loop {
        stack.wait_config_up().await;
        if let Some(config) = stack.config_v4() {
            esp_println::println!("Got IP: {}", config.address);
        }

        stack.wait_config_down().await;
    }
}

#[embassy_executor::task]
async fn task_net(mut runner: embassy_net::Runner<'static, esp_radio::wifi::Interface>) -> ! {
    runner.run().await;
}

#[embassy_executor::task]
async fn task_mmt(mut ctl: esp_radio::wifi::WifiController<'static>) -> ! {
    loop {
        let cred = cap::capture(&mut ctl).await.unwrap();
        esp_println::println!("Received credentials: {} / {}", cred.ssid(), cred.pass());

        let conf = esp_radio::wifi::Config::Station(
            esp_radio::wifi::sta::StationConfig::default()
                .with_ssid(cred.ssid().try_into().unwrap())
                .with_authentication(esp_radio::wifi::AuthenticationMethodConfig::WpaWpa2Personal(
                    cred.pass().try_into().unwrap())));

        ctl.set_config(&conf).unwrap();

        let mut retries = 0;
        loop {
            esp_println::println!("Connecting to {} [{}]...", cred.ssid(), retries + 1);
            match ctl.connect_async().await {
                Ok(_) => {},
                Err(_) => {
                    retries += 1;
                    if retries == 3 {
                        break;
                    }

                    Timer::after_secs(3).await;

                    continue;
                }
            }

            esp_println::println!("Successfully connected to {}", cred.ssid());

            let _ = ctl.wait_for_disconnect_async().await;
        }
    }
}

// #[embassy_executor::task]
// async fn task_ddp(stack: Stack<'static>, mut spi: SpiDma<'static, Async>) -> ! {
//     let mut srv = ddp::Listener::new(stack);
//     let mut pkt = ddp::Packet::default();
//     let mut rx = [0u8; 8];
// 
//     loop {
//         srv.recv(&mut pkt).await;
//         esp_println::println!("Received: {:?}", pkt);
// 
//         let body: &[u8] = unsafe { bytemuck::cast_slice(&pkt.body.rgb) };
//         let tx = &body[..pkt.header.size as usize];
// 
//         SpiBus::transfer(&mut spi, &mut rx, tx).await.unwrap();
//         esp_println::println!("Transfered: {}", tx.len());
//     }
// }

#[embassy_executor::task]
async fn task_ddp(stack: Stack<'static>, mut i2c: I2c<'static, Async>) -> ! {
    let mut srv = ddp::Listener::new(stack);
    let mut pkt = ddp::Packet::default();

    loop {
        srv.recv(&mut pkt).await;
        esp_println::println!("Received: {:?}", pkt);

        let body: &[u8] = unsafe { bytemuck::cast_slice(&pkt.body.rgb) };
        let tx = &body[..pkt.header.size as usize];

        i2c.write_async(DA, tx).await.unwrap();
        esp_println::println!("Transfered: {} {:02x?}", tx.len(), &tx[..12.min(tx.len())]);
    }
}
