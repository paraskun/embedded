use embassy_rp::{
    Peri, clocks::clk_sys_freq, dma, gpio::Pull, pio::{self, Direction::Out, ShiftConfig, ShiftDirection::Left}, pwm::{self, Pwm},
};
use embassy_time::Timer;
use fixed::types::U24F8;
use core::marker::PhantomData;

const SCREEN_MAX: u16 = 1024;

pub struct Screen<'d, PIO: pio::Instance, SLICE: pwm::Slice> {
    dma: dma::Channel<'d>,
    sm0: pio::StateMachine<'d, PIO, 0>,
    counter: Pwm<'d>,
    frame: [u32; SCREEN_MAX as usize],

    _slice: PhantomData<SLICE>,
}

impl<'d, PIO: pio::Instance, SLICE: pwm::Slice> Screen<'d, PIO, SLICE> {
    pub fn new(
        dma: dma::Channel<'d>,
        pio: pio::Pio<'d, PIO>,
        din_pin: Peri<'d, impl pio::PioPin>,
        pwm: Peri<'d, SLICE>,
        pwm_pin: Peri<'d, impl pwm::ChannelBPin<SLICE>>,
    ) -> Self {
        let pio::Pio { mut common, mut sm0, .. } = pio;

        let code = pio::program::pio_file!("pio/ws2812.pio");
        let din = common.make_pio_pin(din_pin);

        let mut cfg = pio::Config::default();
        cfg.use_program(&common.load_program(&code.program), &[&din]);
        cfg.clock_divider = U24F8::from_num(clk_sys_freq() / 1000) / U24F8::from_num(8000);
        cfg.fifo_join = pio::FifoJoin::TxOnly;
        cfg.shift_out = ShiftConfig {
            auto_fill: true,
            threshold: 24,
            direction: Left,
        };

        sm0.set_config(&cfg);
        sm0.set_pin_dirs(Out, &[&din]);
        sm0.set_enable(true);

        let mut pwm_cfg = pwm::Config::default();
        pwm_cfg.divider = 1.into();

        let counter = Pwm::new_input(pwm, pwm_pin, Pull::Down,
            pwm::InputMode::RisingEdge, pwm_cfg);

        Self {
            dma: dma,
            sm0: sm0,
            counter: counter,
            frame: [0; SCREEN_MAX as usize],
            _slice: PhantomData
        }
    }

    pub async fn size(&mut self) -> u16 {
        self.counter.set_counter(0);

        let v: u32 = (5 << 24) | (5 << 16) | (5 << 8);
        for _ in 0..SCREEN_MAX {
            self.sm0.tx().wait_push(v).await;
        }

        Timer::after_millis(50).await;

        SCREEN_MAX - (self.counter.counter() / 24)
    }

    pub async fn push(&mut self, frame: &[u8]) {
        for (i, ch) in frame.chunks_exact(3).enumerate() {
            self.frame[i] = (u32::from(ch[1]) << 24)
                          | (u32::from(ch[0]) << 16)
                          | (u32::from(ch[2]) << 8);
        }

        self.sm0.tx().dma_push(&mut self.dma, &self.frame[..(frame.len() / 3)], false).await;
    }
}
