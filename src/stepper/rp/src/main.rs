#![no_std]
#![no_main]

extern crate alloc;
use embedded_alloc::LlffHeap as Heap;

use embassy_executor::Spawner;
use embassy_rp::clocks::clk_sys_freq;
use embassy_rp::{dma, peripherals, pio};
use fixed::types::U24F8;

use core::default::Default;
use core::convert::Into;
use core::time::Duration;

use panic_halt as _;

#[global_allocator]
static HEAP: Heap = Heap::empty();

const PIO_FREQ_HZ: u32 = 2_000_000;
const PIO_CLOCK_PERIOD_NS: u32 = 1_000_000_000 / PIO_FREQ_HZ;
const PIO_CLOCKS_PER_DELAY: u32 = 20;
const PIO_DELAY_NS: u32 = PIO_CLOCK_PERIOD_NS * PIO_CLOCKS_PER_DELAY;

#[derive(Clone)]
enum Direction {
    Clockwise,
    CounterClockwise,
}

enum Command {
    Step(Direction),
    Hold,
}

#[derive(Clone)]
struct Delay {
    value: u32,
}

impl Into<Delay> for Duration {
    fn into(self) -> Delay {
        let ticks = self.as_nanos() / PIO_DELAY_NS as u128;
        assert!(ticks > 0 && ticks <= 0x3FFF_FFFF, "delay is out of range");
        Delay { value: ticks as u32 }
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
    unsafe {
        embedded_alloc::init!(HEAP, 256 * 1024);
    }

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

    let mut engine = rhai::Engine::new();
    let ast = engine
        .register_fn("sin", libm::sin)
        .register_fn("cos", libm::cos)
        .compile("
            fn f(t) {
                180 * sin(t)
            }
        ").unwrap();

    let mut scope = rhai::Scope::new();

    let t0 = 0.0;
    let t1 = 2.0 * core::f64::consts::PI;
    let dt = (t1 - t0) / 1000.0;

    let mut i = 0;
    let f0 = engine.call_fn::<f64>(&mut scope, &ast, "f", (t0,)).unwrap();
    let mut position = libm::round(f0 / 0.225) as i32;
    let initial = position;
    let mut elapsed_ticks = 0;
    let mut program: [u32; 4096] = [0; 4096];

    for sample in 1..=1000 {
        let t = t0 + dt * sample as f64;
        let f1 = engine.call_fn::<f64>(&mut scope, &ast, "f", (t,)).unwrap();
        let target = libm::round(f1 / 0.225);
        assert!(target.is_finite() && target >= i32::MIN as f64 && target <= i32::MAX as f64);
        let target = target as i32;
        let df = i64::from(target) - i64::from(position);
        let n = df.unsigned_abs();

        // Absolute deadlines carry fractional ticks across sample intervals.
        let deadline = libm::round((t - t0) * 1_000_000_000.0 / PIO_DELAY_NS as f64) as u64;
        let ticks = deadline - elapsed_ticks;
        assert!(n.max(1) <= (program.len() - i) as u64, "program buffer is full");

        if n == 0 {
            program[i] = Instruction::new(
                Command::Hold,
                Duration::from_nanos(ticks * PIO_DELAY_NS as u64).into()
            ).into();
            i += 1;
        } else {
            assert!(ticks >= n, "trajectory is too fast");
            let dir = if df < 0 { Direction::Clockwise } else { Direction::CounterClockwise };

            for j in 0..n {
                let d = ticks / n + u64::from(j < ticks % n);
                program[i] = Instruction::new(
                    Command::Step(dir.clone()),
                    Duration::from_nanos(d * PIO_DELAY_NS as u64).into()
                ).into();
                i += 1;
            }
        }

        position = target;
        elapsed_ticks = deadline;
    }

    loop {
        pio.sm0.tx().dma_push(&mut dma, &program[..i], false).await;
    }
}
