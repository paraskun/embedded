#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::clocks::clk_sys_freq;
use embassy_rp::{dma, peripherals, pio};
use fixed::types::U24F8;

use core::default::Default;
use core::convert::Into;
use core::time::Duration;

use panic_halt as _;

const PIO_FREQ_HZ: u32 = 2_000_000;
const PIO_CLOCK_PERIOD_NS: u32 = 1_000_000_000 / PIO_FREQ_HZ;
const PIO_CLOCKS_PER_DELAY: u32 = 20;
const PIO_DELAY_NS: u32 = PIO_CLOCK_PERIOD_NS * PIO_CLOCKS_PER_DELAY;

enum Direction {
    Clockwise,
    CounterClockwise,
}

enum Command {
    Step(Direction),
    Hold,
}

struct Delay {
    value: u32,
}

impl Delay {
    fn max() -> Self {
        0x3FFF_FFFF.into()
    }
}

impl Into<u32> for Delay {
    fn into(self) -> u32 {
        self.value
    }
}

impl Into<Delay> for Duration {
    fn into(self) -> Delay {
        let nanos = self.as_nanos();
        let ticks = nanos / PIO_DELAY_NS as u128;

        if ticks > 0x3FFF_FFFF {
            Delay::max()
        } else {
            Delay { value: ticks as u32 }
        }
    }
}

impl Into<Delay> for u32 {
    fn into(self) -> Delay {
        Delay { value: self & 0x3FFF_FFFF }
    }
}

struct Instruction {
    command: Command,
    delay: Delay,
}

impl Instruction {
    fn new(command: Command, delay: Delay) -> Self {
        Self { command, delay }
    }
}

impl Into<u32> for Instruction {
    fn into(self) -> u32 {
        let cmd = match self.command {
            Command::Step(Direction::CounterClockwise) => 0b01,
            Command::Step(Direction::Clockwise) => 0b00,
            Command::Hold => 0b10,
        };

        (cmd << 30) | (self.delay.value & 0x3FFF_FFFF)
    }
}

embassy_rp::bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => pio::InterruptHandler<peripherals::PIO0>;
    DMA_IRQ_0 => dma::InterruptHandler<peripherals::DMA_CH0>;
});

#[embassy_executor::main(
    executor = "embassy_rp::executor::Executor",
    entry = "cortex_m_rt::entry",
)]
async fn main(_spawner: Spawner) {
    let hal = embassy_rp::init(Default::default());

    let mut pio = pio::Pio::new(hal.PIO0, Irqs);
    let mut dma = dma::Channel::new(hal.DMA_CH0, Irqs);

    let code = pio::program::pio_file!("pio/stepper.pio");
    let step_pin = pio.common.make_pio_pin(hal.PIN_6);
    let dir_pin = pio.common.make_pio_pin(hal.PIN_7);

    let mut cfg = pio::Config::default();
    cfg.use_program(&pio.common.load_program(&code.program), &[&step_pin]);
    cfg.set_out_pins(&[&dir_pin]);
    cfg.clock_divider = U24F8::from_num(
        clk_sys_freq() as f32 /
        PIO_FREQ_HZ as f32);
    cfg.fifo_join = pio::FifoJoin::TxOnly;
    cfg.shift_out = pio::ShiftConfig {
        auto_fill: false,
        threshold: 32,
        direction: pio::ShiftDirection::Left,
    };

    pio.sm0.set_config(&cfg);
    pio.sm0.set_pin_dirs(pio::Direction::Out, &[&step_pin, &dir_pin]);
    pio.sm0.set_enable(true);

    let steps: [u32; 128] = core::array::from_fn(|_| {
        Instruction::new(
            Command::Step(Direction::Clockwise),
            Duration::from_millis(10).into(),
        ).into()
    });

    loop {
        pio.sm0.tx().dma_push(&mut dma, &steps, false).await;
    }
}
