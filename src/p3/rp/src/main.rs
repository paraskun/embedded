#![no_std]
#![no_main]

mod pio;
mod usb;

use embassy_executor::Spawner;
use embassy_rp::{
    config::Config, dma, i2c, peripherals::{self, DMA_CH0}, pio as rp_pio, usb::Driver,
};
use embassy_usb::UsbDevice;
use panic_halt as _;

embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => rp_pio::InterruptHandler<peripherals::PIO0>;
    I2C0_IRQ => i2c::InterruptHandler<peripherals::I2C0>;
    USBCTRL_IRQ => embassy_rp::usb::InterruptHandler<peripherals::USB>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>;
});

#[embassy_executor::task]
async fn usb_task(mut device: UsbDevice<'static, Driver<'static, peripherals::USB>>) {
    device.run().await;
}

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry",
)]
async fn main(_spawner: Spawner) {
    let hal = embassy_rp::init(Config::default());

    let pio0 = rp_pio::Pio::new(hal.PIO0, Irqs);
    let mut screen = pio::Screen::new(
        dma::Channel::new(hal.DMA_CH0, Irqs),
        pio0, 
        hal.PIN_19,
        hal.PWM_SLICE0, 
        hal.PIN_1
    );

    let mut iface = usb::Interface::new(Driver::new(hal.USB, Irqs));

    let mut buf = [0u32, 64];
    let mut frame = [0u8; 192];

    loop {
        match iface.read(&mut frame).await {
            usb::Cmd::Size => {
                let size = screen.size().await;
                iface.write(&size.to_be_bytes()).await
            },
            usb::Cmd::Frame => {
                let mut i = 0;

                for ch in frame.chunks_exact(3) {
                    buf[i] = (u32::from(ch[1]) << 24)
                            | (u32::from(ch[0]) << 16)
                            | (u32::from(ch[2]) << 8);

                    i += 1;
                }

                screen.push(&buf).await;
            },
        }
    }
}
