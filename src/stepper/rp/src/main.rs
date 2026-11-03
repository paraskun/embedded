#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::{
    clocks::clk_sys_freq,
    dma,
    gpio::Level,
    peripherals,
    pio::{self, Direction::Out, ShiftConfig, ShiftDirection},
};
use embassy_time::Duration;
use fixed::types::U24F8;
use panic_halt as _;

const MAX_DELAY_TICKS: u128 = 0x7fff_ffff;

#[derive(Clone, Copy)]
#[repr(u32)]
enum StepDirection {
    CounterClockwise = 0,
    Clockwise = 1,
}

fn calculate_divider(accuracy: Duration) -> U24F8 {
    let raw = u128::from(clk_sys_freq()) * u128::from(accuracy.as_nanos()) * 256 / 1_000_000_000;
    assert!(raw >= 256, "accuracy exceeds PIO divider resolution");
    U24F8::from_bits(raw.min(1 << 24) as u32)
}

fn instruction(divider: U24F8, direction: StepDirection, delay: Duration) -> u32 {
    let numerator = u128::from(delay.as_nanos()) * u128::from(clk_sys_freq()) * 256;
    let denominator = u128::from(divider.to_bits()) * 1_000_000_000;
    let ticks = (numerator + denominator / 2) / denominator;
    assert!(
        ticks <= MAX_DELAY_TICKS,
        "delay exceeds PIO instruction range"
    );
    ((ticks as u32) << 1) | direction as u32
}

embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<peripherals::DMA_CH0>;
});

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry"
)]
async fn main(_spawner: Spawner) {
    let hal = embassy_rp::init(Default::default());
    let pio::Pio {
        mut common,
        mut sm0,
        ..
    } = pio::Pio::new(hal.PIO0, Irqs);
    let mut dma = dma::Channel::new(hal.DMA_CH0, Irqs);

    let code = pio::program::pio_file!("pio/stepper.pio");
    let program = common.load_program(&code.program);
    let pins = [
        common.make_pio_pin(hal.PIN_6),
        common.make_pio_pin(hal.PIN_7),
        common.make_pio_pin(hal.PIN_8),
        common.make_pio_pin(hal.PIN_9),
    ];
    let pin_refs = [&pins[0], &pins[1], &pins[2], &pins[3]];
    let divider = calculate_divider(Duration::from_millis(100));

    let mut cfg = pio::Config::default();
    cfg.use_program(&program, &pin_refs);
    cfg.clock_divider = divider;
    cfg.fifo_join = pio::FifoJoin::TxOnly;
    cfg.shift_out = ShiftConfig {
        auto_fill: false,
        threshold: 32,
        direction: ShiftDirection::Right,
    };
    cfg.shift_in = ShiftConfig {
        auto_fill: false,
        threshold: 32,
        direction: ShiftDirection::Left,
    };

    sm0.set_config(&cfg);
    sm0.set_pin_dirs(Out, &pin_refs);
    sm0.set_pins(Level::Low, &pin_refs);
    sm0.set_enable(true);

    let mut instructions = [0u32; 256];
    instructions[..128].fill(instruction(
        divider,
        StepDirection::Clockwise,
        Duration::from_millis(2),
    ));
    instructions[128..].fill(instruction(
        divider,
        StepDirection::CounterClockwise,
        Duration::from_millis(500),
    ));

    loop {
        sm0.tx().dma_push(&mut dma, &instructions, false).await;
    }
}
