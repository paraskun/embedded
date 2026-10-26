#![no_std]
#![no_main]

mod led;

use embassy_executor::Spawner;
use embassy_rp::{dma, i2c, i2c_slave, peripherals, pio};
use panic_halt as _;

const DA: u8 = 0x42;

embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<peripherals::DMA_CH0>;
    I2C0_IRQ => i2c::InterruptHandler<peripherals::I2C0>;
});

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry",
)]
async fn main(_spawner: Spawner) {
    let hal = embassy_rp::init(Default::default());

    let mut led = led::Screen::new(
        dma::Channel::new(hal.DMA_CH0, Irqs),
        pio::Pio::new(hal.PIO0, Irqs),
        hal.PIN_13,
        hal.PWM_SLICE0,
        hal.PIN_1);

    let mut cfg = i2c_slave::Config::default();
    cfg.addr = DA as u16;

    let mut dev = i2c_slave::I2cSlave::new(
        hal.I2C0,
        hal.PIN_5,
        hal.PIN_4,
        Irqs,
        cfg);

    let mut frame = [0u8; 192];

    loop {
        match dev.listen(&mut frame).await {
            Ok(i2c_slave::Command::Write(n)) => led.push(&frame[..n]).await,
            Err(i2c_slave::Error::PartialWrite(n)) => led.push(&frame[..n]).await,
            _ => (),
        }
    }
}
