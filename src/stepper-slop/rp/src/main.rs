#![no_std]
#![no_main]

use core::pin::pin;

use embassy_executor::Spawner;
use embassy_rp::{
    bind_interrupts,
    config::Config,
    dma,
    peripherals::{self, DMA_CH0, DMA_CH1},
    pio as rp_pio,
};
use panic_halt as _;
use stepper_rp::{
    Action, Command, CommandBuffer, DelayUnits, Direction, DmaPair, PioResources, Stepper,
    TimingConfig,
};

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => rp_pio::InterruptHandler<peripherals::PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<DMA_CH0>, dma::InterruptHandler<DMA_CH1>;
});

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: Spawner) {
    let hal = embassy_rp::init(Config::default());

    // TMC2209 prerequisites are board setup work: external STEP/DIR mode,
    // CHOPCONF.dedge=0, VACTUAL=0, fixed microsteps/current, and safe wiring.
    let timing = match TimingConfig::for_sys_clock(embassy_rp::clocks::clk_sys_freq()) {
        Ok(timing) => timing,
        Err(_) => panic!("unsupported system clock for exact 2 MHz PIO timing"),
    };

    let rp_pio::Pio {
        mut common, sm0, ..
    } = rp_pio::Pio::new(hal.PIO0, Irqs);
    let pio = match PioResources::new(&mut common, sm0, hal.PIN_6, hal.PIN_7, timing) {
        Ok(pio) => pio,
        Err(_) => panic!("could not load or configure STEP/DIR PIO program"),
    };
    let dma = DmaPair::new(hal.DMA_CH0, hal.DMA_CH1, Irqs, Irqs);

    // One step followed by a full-slot hold at 1 ms per command. The driver
    // owns stable DMA storage; pin it before submitting any command words.
    let mut motor = pin!(Stepper::<_, _, 2>::new(pio, dma));
    let delay = match DelayUnits::new(100) {
        Ok(delay) => delay,
        Err(_) => panic!("invalid conservative demo delay"),
    };
    let commands = CommandBuffer::new([
        Command::new(Action::Step, Direction::Low, delay),
        Command::new(Action::Hold, Direction::Low, delay),
    ]);

    if motor.as_mut().queue(commands).is_err() {
        panic!("failed to queue the first demo buffer");
    }
    let continuation = CommandBuffer::new([
        Command::new(Action::Hold, Direction::Low, delay),
        Command::new(Action::Hold, Direction::Low, delay),
    ]);
    if motor.as_mut().queue(continuation).is_err() {
        panic!("failed to queue the chained demo buffer");
    }
    if motor.as_mut().finish().is_err() {
        panic!("failed to seal the conservative demo stream");
    }
    if motor.as_mut().start().is_err() {
        panic!("failed to start the conservative demo stream");
    }
    motor.as_mut().wait_idle().await;

    loop {
        // Hold the driver state after the finite stream has drained.
        core::future::pending::<()>().await;
    }
}
