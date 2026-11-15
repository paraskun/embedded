//! Embassy PIO backend for the fixed STEP/DIR command stream.
//!
//! The caller selects a PIO block and state machine, owns DMA channels, and
//! submits complete command words through the typed TX FIFO handle returned
//! by [`PioResources::tx`].

use embassy_rp::{
    Peri,
    gpio::Level,
    pio::{self, Direction, ShiftConfig, ShiftDirection},
};

use crate::proto::TimingConfig;

/// GPIO assignment in the initial RP2350 wiring contract.
pub const STEP_GPIO: u8 = 6;
/// GPIO assignment in the initial RP2350 wiring contract.
pub const DIRECTION_GPIO: u8 = 7;

/// Owned PIO resources for one stepper channel.
///
/// `Common` is borrowed only during construction, so the caller can keep the
/// PIO block available for other state machines. The selected state machine,
/// pin handles, and installed instruction memory remain owned here.
pub struct PioResources<'d, PIO: pio::Instance, const SM: usize> {
    sm: pio::StateMachine<'d, PIO, SM>,
    step: pio::Pin<'d, PIO>,
    direction: pio::Pin<'d, PIO>,
    program: pio::LoadedProgram<'d, PIO>,
    config: pio::Config<'d, PIO>,
}

/// Descriptive alias for one configured stepper state machine.
pub type PioStepper<'d, PIO, const SM: usize> = PioResources<'d, PIO, SM>;

impl<'d, PIO: pio::Instance, const SM: usize> PioResources<'d, PIO, SM> {
    /// Load and configure the stepper program, leaving the state machine off.
    ///
    /// The demo passes GPIO6 (STEP) and GPIO7 (DIR). Board integration owns
    /// the choice and wiring of the pin peripherals supplied here.
    pub fn new(
        common: &mut pio::Common<'d, PIO>,
        mut sm: pio::StateMachine<'d, PIO, SM>,
        step: Peri<'d, impl pio::PioPin>,
        direction: Peri<'d, impl pio::PioPin>,
        timing: TimingConfig,
    ) -> Result<Self, pio::LoadError> {
        // The caller may hand us an SM that was previously in use. Make all
        // subsequent pin/configuration changes while execution is stopped.
        sm.set_enable(false);
        let code = pio::program::pio_file!("pio/stepper.pio");
        let program = common.try_load_program(&code.program)?;
        let step = common.make_pio_pin(step);
        let direction = common.make_pio_pin(direction);

        let mut config = pio::Config::default();
        config.use_program(&program, &[&step]);
        config.set_out_pins(&[&direction]);
        // The program does not execute SET instructions. Mapping both pins
        // here lets Embassy's set_pins helper force their initial/reset values.
        config.set_set_pins(&[&step, &direction]);
        config.clock_divider = timing.divider();
        config.fifo_join = pio::FifoJoin::TxOnly;
        config.shift_out = ShiftConfig {
            auto_fill: false,
            threshold: 32,
            direction: ShiftDirection::Left,
        };

        sm.set_config(&config);
        sm.set_pin_dirs(Direction::Out, &[&step, &direction]);
        sm.set_pins(Level::Low, &[&step, &direction]);
        sm.set_enable(false);

        Ok(Self {
            sm,
            step,
            direction,
            program,
            config,
        })
    }

    /// Return the typed TX FIFO endpoint for CPU or DMA submission.
    ///
    pub fn tx(&mut self) -> &mut pio::StateMachineTx<'d, PIO, SM> {
        self.sm.tx()
    }

    /// Return the selected state machine's raw TX FIFO address and DREQ for
    /// the chained DMA runtime. Embassy 0.10 keeps these accessors on
    /// `StateMachine`; `StateMachineTx::dreq()` itself is private.
    pub(crate) fn dma_endpoint(&self) -> (*mut u32, embassy_rp::pac::dma::vals::TreqSel) {
        (self.sm.tx_fifo_ptr(), self.sm.tx_treq())
    }

    /// Enable execution after the caller has primed the TX FIFO.
    pub fn start(&mut self) {
        let _ = self.sm.tx().stalled();
        self.sm.set_enable(true);
    }

    /// Disable PIO execution and force STEP low, preserving the current DIR.
    ///
    /// If called during a high pulse, it truncates the pulse and leaves motor
    /// position uncertain; callers must treat this as an abort.
    pub fn stop(&mut self) {
        self.sm.set_enable(false);
        self.sm.set_pins(Level::Low, &[&self.step]);
    }

    /// Clear both FIFOs while keeping the state machine disabled.
    pub fn clear(&mut self) {
        self.sm.set_enable(false);
        self.sm.clear_fifos();
        // `stalled()` reads and clears the hardware's sticky TXSTALL bit.
        let _ = self.sm.tx().stalled();
    }

    /// Reset execution to the blocking pull with both output pins low.
    ///
    /// The SM remains disabled so a caller can prime a fresh stream before
    /// enabling it. Re-applying the full config restores the program origin,
    /// shift direction, divider, and TX-only FIFO settings.
    pub fn reset_to_pull(&mut self) {
        self.sm.set_enable(false);
        self.sm.clear_fifos();
        self.sm.set_config(&self.config);
        self.sm
            .set_pin_dirs(Direction::Out, &[&self.step, &self.direction]);
        self.sm.set_pins(Level::Low, &[&self.step, &self.direction]);
        let _ = self.sm.tx().stalled();
    }

    /// Whether the TX FIFO is empty. This does not imply the active command is
    /// complete; the state machine may still be executing it.
    pub fn tx_empty(&mut self) -> bool {
        self.sm.tx().empty()
    }

    /// Snapshot whether the enabled SM has an empty FIFO and its PC at pull.
    ///
    /// A snapshot can catch the boundary before PULL executes. The runtime
    /// confirms it only after DMA is quiescent and the snapshot remains true
    /// across a 10 microsecond interval; a historical TXSTALL alone cannot
    /// establish completion.
    pub fn pull_blocked(&mut self) -> bool {
        let pull_pc = self.program.origin;
        let pc_before = self.sm.get_addr();
        let tx_empty = self.sm.tx().empty();
        let pc_after = self.sm.get_addr();
        self.sm.is_enabled() && tx_empty && pc_before == pull_pc && pc_after == pull_pc
    }

    /// Read and clear the sticky TXSTALL event bit.
    ///
    /// This reports whether the SM stalled at an empty pull at least once since
    /// the previous call. It is diagnostic history, not completion proof.
    pub fn take_stall_status(&mut self) -> bool {
        self.sm.tx().stalled()
    }

    /// Clear any historical empty-pull event before checking a new service
    /// interval. Call after a refill has completed if the runtime needs to
    /// distinguish that refill from a later underrun.
    pub fn clear_stall_status(&mut self) {
        let _ = self.sm.tx().stalled();
    }

    /// Current number of words queued in the joined TX FIFO.
    pub fn tx_level(&mut self) -> u8 {
        self.sm.tx().level()
    }
}

impl<'d, PIO: pio::Instance, const SM: usize> Drop for PioResources<'d, PIO, SM> {
    fn drop(&mut self) {
        self.sm.set_enable(false);
        self.sm.clear_fifos();
        self.sm.set_pins(Level::Low, &[&self.step, &self.direction]);
    }
}
