#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::{gpio::{self, Level}};
use embassy_time::Timer;
use panic_halt as _;

embassy_rp::bind_interrupts!(struct Irqs {
    // PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
    // DMA_IRQ_0 => dma::InterruptHandler<peripherals::DMA_CH0>;
    // I2C0_IRQ => i2c::InterruptHandler<peripherals::I2C0>;
});

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry",
)]
async fn main(_spawner: Spawner) {
    let hal = embassy_rp::init(Default::default());

    let mut c = [
        gpio::Output::new(hal.PIN_18, Level::Low),
        gpio::Output::new(hal.PIN_19, Level::Low),
        gpio::Output::new(hal.PIN_20, Level::Low),
        gpio::Output::new(hal.PIN_21, Level::Low),
    ];

    let mut a = 0;
    let mut b = 0;

    loop {
        c[a].set_low();
        c[b].set_low();

        if a == b {
            b += 1;

            if b == 4 {
                b = 0;
            }
        } else {
            a = b;
        }

        c[a].set_high();
        c[b].set_high();

        Timer::after_millis(2).await;
    }
}
