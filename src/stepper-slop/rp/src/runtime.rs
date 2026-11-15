//! Bounded DMA queue and lifecycle for one STEP/DIR PIO state machine.
//!
//! The driver owns two DMA channels and two command buffers. The buffers stay
//! at stable addresses while DMA can read them; every operation that can expose
//! or replace a buffer takes a pinned receiver. The caller must pin the
//! [`Stepper`] before starting it and keep the system clock fixed at the
//! validated 2 MHz PIO rate while it runs.

use core::{
    marker::PhantomPinned,
    pin::Pin,
    sync::atomic::{Ordering, compiler_fence},
};

use embassy_rp::{
    Peri, dma,
    interrupt::typelevel::{self, Binding},
    pac, peripherals,
};
use embassy_time::{Instant, Timer};

use crate::{proto::Command, stepper::PioResources};

const CHANNELS: [u8; 2] = [0, 1];
const MAX_DMA_TRANSFERS: usize = 0x0fff_ffff;
const PULL_CONFIRM_US: u64 = 10;

/// A fixed-size buffer of validated commands. Commands are immutable after
/// construction; submitting one moves it into the pinned runtime. A zero-sized
/// instance is rejected by `queue`.
pub struct CommandBuffer<const N: usize> {
    words: [Command; N],
}

impl<const N: usize> CommandBuffer<N> {
    pub fn new(commands: [Command; N]) -> Self {
        Self { words: commands }
    }

    pub const fn len(&self) -> usize {
        N
    }

    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

/// Opaque identity for a submitted command buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Submission(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueError {
    Full,
    EmptyBuffer,
    InvalidState,
    StreamClosed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Prepared,
    Running,
    Idle,
    Aborted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    pub state: RunState,
    /// Sticky diagnostic: a TX FIFO starvation event was observed at least
    /// once since construction or the most recent reset.
    pub tx_stall_observed: bool,
    /// Sticky indication that an unsealed running stream starved before a
    /// refill. A sealed final pull does not count as an underrun.
    pub underrun_observed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbortReport {
    pub position_uncertain: bool,
}

/// The two hardware channels reserved for ping-pong DMA. The first argument
/// must own DMA_CH0 and the second DMA_CH1; on RP2350 these both use DMA_IRQ_0.
pub struct DmaPair<'d> {
    channels: [dma::Channel<'d>; 2],
}

impl<'d> DmaPair<'d> {
    /// Own DMA_CH0 and DMA_CH1, binding both channels to DMA_IRQ_0.
    pub fn new<I0, I1>(
        channel0: Peri<'d, peripherals::DMA_CH0>,
        channel1: Peri<'d, peripherals::DMA_CH1>,
        irq0: I0,
        irq1: I1,
    ) -> Self
    where
        I0: Binding<typelevel::DMA_IRQ_0, dma::InterruptHandler<peripherals::DMA_CH0>> + 'd,
        I1: Binding<typelevel::DMA_IRQ_0, dma::InterruptHandler<peripherals::DMA_CH1>> + 'd,
    {
        Self {
            channels: [
                dma::Channel::new(channel0, irq0),
                dma::Channel::new(channel1, irq1),
            ],
        }
    }

    fn channel(&self, index: usize) -> &dma::Channel<'d> {
        &self.channels[index]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotState {
    Free,
    Queued,
    Armed,
    Active,
    DmaDone,
}

struct Slot<const N: usize> {
    buffer: Option<CommandBuffer<N>>,
    state: SlotState,
    submission: Submission,
    channel: usize,
}

impl<const N: usize> Slot<N> {
    const fn new() -> Self {
        Self {
            buffer: None,
            state: SlotState::Free,
            submission: Submission(0),
            channel: 0,
        }
    }
}

/// A two-slot, hardware-chained command transport and motion lifecycle.
///
/// At `D=1`, a slot containing `N` commands provides at most `10*N` µs before
/// its successor must be armed, excluding the command already executing. Keep
/// DMA/CPU service latency below that margin. Cancellation of [`wait_idle`]
/// leaves the stream running.
pub struct Stepper<'d, PIO: pio::Instance, const SM: usize, const N: usize> {
    pio: PioResources<'d, PIO, SM>,
    dma: DmaPair<'d>,
    slots: [Slot<N>; 2],
    order: [Option<usize>; 2],
    order_len: usize,
    next_channel: usize,
    next_submission: u32,
    state: RunState,
    sealed: bool,
    submitted_any: bool,
    stall_seen: bool,
    underrun_seen: bool,
    drain_candidate: Option<Instant>,
    position_uncertain: bool,
    _pin: PhantomPinned,
}

impl<'d, PIO: pio::Instance, const SM: usize, const N: usize> Stepper<'d, PIO, SM, N> {
    pub fn new(pio: PioResources<'d, PIO, SM>, dma: DmaPair<'d>) -> Self {
        Self {
            pio,
            dma,
            slots: [Slot::new(), Slot::new()],
            order: [None, None],
            order_len: 0,
            next_channel: 0,
            next_submission: 0,
            state: RunState::Prepared,
            sealed: false,
            submitted_any: false,
            stall_seen: false,
            underrun_seen: false,
            drain_candidate: None,
            position_uncertain: false,
            _pin: PhantomPinned,
        }
    }

    /// Submit a complete immutable buffer. The first two outstanding buffers
    /// occupy the two DMA-owned slots; a third is returned with `Full`.
    pub fn queue(
        self: Pin<&mut Self>,
        buffer: CommandBuffer<N>,
    ) -> Result<Submission, (QueueError, CommandBuffer<N>)> {
        // SAFETY: fields containing DMA source memory are never moved while the
        // object is pinned. This method only moves a buffer into a free slot;
        // all existing DMA pointers continue to refer to their original slot.
        let this = unsafe { self.get_unchecked_mut() };
        if N == 0 {
            return Err((QueueError::EmptyBuffer, buffer));
        }
        if N > MAX_DMA_TRANSFERS {
            return Err((QueueError::InvalidState, buffer));
        }
        if this.state == RunState::Aborted {
            return Err((QueueError::InvalidState, buffer));
        }
        if this.sealed {
            return Err((QueueError::StreamClosed, buffer));
        }
        if this.state == RunState::Idle {
            return Err((QueueError::InvalidState, buffer));
        }
        if this.order_len == 2 {
            return Err((QueueError::Full, buffer));
        }

        let slot_index = match this
            .slots
            .iter()
            .position(|slot| slot.state == SlotState::Free)
        {
            Some(index) => index,
            None => return Err((QueueError::Full, buffer)),
        };

        // Preserve diagnostics before clearing the hardware's sticky bit. A
        // refill may race the next status poll, so latch unsealed underruns
        // here as well.
        let stalled = this.pio.take_stall_status();
        this.stall_seen |= stalled;
        if stalled && this.state == RunState::Running && !this.sealed {
            this.underrun_seen = true;
        }
        this.drain_candidate = None;
        let channel = this.next_channel;
        this.next_channel ^= 1;
        let submission = Submission(this.next_submission);
        this.next_submission = this.next_submission.wrapping_add(1);
        this.slots[slot_index] = Slot {
            buffer: Some(buffer),
            state: SlotState::Queued,
            submission,
            channel,
        };
        this.order[this.order_len] = Some(slot_index);
        this.order_len += 1;
        this.submitted_any = true;

        if this.state == RunState::Running {
            this.refresh_dma();
            let has_prior_transfer = this.order[..this.order_len - 1]
                .iter()
                .flatten()
                .any(|&i| matches!(this.slots[i].state, SlotState::Armed | SlotState::Active));
            if has_prior_transfer {
                this.configure_slot(slot_index, CHANNELS[channel], false);
                this.slots[slot_index].state = SlotState::Armed;
                let predecessor = this.order[this.order_len - 2].expect("previous queued slot");
                let predecessor_channel = this.slots[predecessor].channel;
                this.set_chain_target(predecessor_channel, CHANNELS[channel]);

                // The predecessor may have completed just before its chain
                // target changed. In that case the target is still unstarted
                // and must be kicked by software. An armed predecessor waiting
                // on an earlier channel is deliberately left alone.
                let predecessor_was_started = this.slots[predecessor].state == SlotState::Active;
                if predecessor_was_started && !this.channel_busy(predecessor_channel) {
                    this.trigger_if_unstarted(slot_index, predecessor_channel);
                } else if !predecessor_was_started {
                    let earlier_in_flight =
                        this.order[..this.order_len - 2].iter().flatten().any(|&i| {
                            matches!(this.slots[i].state, SlotState::Armed | SlotState::Active)
                        });
                    if !earlier_in_flight {
                        this.trigger_predecessor_if_unstarted(predecessor, CHANNELS[channel]);
                    }
                }
            } else {
                this.configure_slot(slot_index, CHANNELS[channel], true);
                this.slots[slot_index].state = SlotState::Active;
            }
        }
        Ok(submission)
    }

    /// Seal the current nonempty stream. Later submissions are rejected.
    pub fn finish(self: Pin<&mut Self>) -> Result<(), QueueError> {
        // SAFETY: no pinned storage is moved or exposed mutably.
        let this = unsafe { self.get_unchecked_mut() };
        if !matches!(this.state, RunState::Prepared | RunState::Running) {
            return Err(QueueError::InvalidState);
        }
        if this.sealed {
            return Err(QueueError::StreamClosed);
        }
        if !this.submitted_any {
            return Err(QueueError::EmptyBuffer);
        }
        let stalled = this.pio.take_stall_status();
        this.stall_seen |= stalled;
        if stalled && this.state == RunState::Running {
            this.underrun_seen = true;
        }
        this.sealed = true;
        this.drain_candidate = None;
        Ok(())
    }

    /// Start a prepared stream, priming TX before enabling the state machine.
    pub fn start(self: Pin<&mut Self>) -> Result<(), QueueError> {
        // SAFETY: the runtime remains pinned and no DMA source is moved.
        let this = unsafe { self.get_unchecked_mut() };
        if this.state != RunState::Prepared {
            return Err(QueueError::InvalidState);
        }
        if this.order_len == 0 {
            return Err(QueueError::EmptyBuffer);
        }

        // Arm the successor first, so channel 0 can never finish before its
        // chain target has been prepared.
        if this.order_len == 2 {
            let successor = this.order[1].expect("second queued slot");
            let first = this.order[0].expect("first queued slot");
            let first_channel = this.slots[first].channel;
            let successor_channel = this.slots[successor].channel;
            this.configure_slot(successor, CHANNELS[successor_channel], false);
            this.slots[successor].state = SlotState::Armed;
            this.set_chain_target(first_channel, CHANNELS[successor_channel]);
        }
        let first = this.order[0].expect("first queued slot");
        let first_channel = this.slots[first].channel;
        let first_chain = if this.order_len == 2 {
            CHANNELS[this.slots[this.order[1].expect("successor")].channel]
        } else {
            CHANNELS[first_channel]
        };
        this.configure_slot(first, first_chain, true);
        this.slots[first].state = SlotState::Active;

        // DREQ can transfer into the disabled SM's FIFO. Wait for the first
        // word before enabling PIO; this is a bounded hardware handshake, not
        // a transfer-completion wait.
        while this.pio.tx_level() == 0 && this.channel_busy(this.slots[first].channel) {}
        if this.pio.tx_level() == 0 {
            this.refresh_dma();
            if this.slots[first].state == SlotState::DmaDone {
                return Err(QueueError::InvalidState);
            }
        }
        this.pio.start();
        this.state = RunState::Running;
        this.drain_candidate = None;
        Ok(())
    }

    /// Return the oldest buffer whose final DMA read has completed. This says
    /// nothing about whether its commands have finished executing.
    pub fn try_reclaim(self: Pin<&mut Self>) -> Option<(Submission, CommandBuffer<N>)> {
        // SAFETY: completed slots have no in-flight DMA pointer; active slots
        // are not moved or returned.
        let this = unsafe { self.get_unchecked_mut() };
        this.refresh_dma();
        let slot_index = this.order.first().copied().flatten()?;
        if this.slots[slot_index].state != SlotState::DmaDone {
            return None;
        }
        this.remove_order(0);
        let slot = &mut this.slots[slot_index];
        let submission = slot.submission;
        let buffer = slot.buffer.take()?;
        slot.state = SlotState::Free;
        Some((submission, buffer))
    }

    /// Snapshot lifecycle and starvation diagnostics. A sealed stream becomes
    /// Idle only after DMA quiescence and a stable empty blocking-pull state.
    pub fn status(self: Pin<&mut Self>) -> Status {
        // SAFETY: status updates bookkeeping only; it does not move pinned
        // command storage.
        let this = unsafe { self.get_unchecked_mut() };
        let stalled = this.pio.take_stall_status();
        this.stall_seen |= stalled;
        if stalled && this.state == RunState::Running && !this.sealed {
            this.underrun_seen = true;
        }
        this.refresh_dma();

        if this.state == RunState::Running && this.sealed && this.dma_quiescent() {
            let at_empty_pull = this.pio.tx_empty() && this.pio.pull_blocked();
            if at_empty_pull {
                let elapsed_us = this
                    .drain_candidate
                    .map(|since| Instant::now().duration_since(since).as_micros());
                match drain_action(elapsed_us) {
                    DrainAction::StartCandidate => {
                        // The status read at the start of this observation
                        // consumed historical TXSTALL. The 10 µs confirmation
                        // interval lets a final-NOP-to-PULL boundary settle.
                        this.drain_candidate = Some(Instant::now());
                    }
                    DrainAction::Confirm => {
                        // No producer or refill can invalidate this stable
                        // PC0/empty observation. A one-buffer stream is valid:
                        // Free slots count as quiescent.
                        if this.pio.tx_empty() && this.pio.pull_blocked() && this.dma_quiescent() {
                            this.state = RunState::Idle;
                        }
                    }
                    DrainAction::Wait => {}
                }
            } else {
                this.drain_candidate = None;
            }
        } else {
            this.drain_candidate = None;
        }

        Status {
            state: this.state,
            tx_stall_observed: this.stall_seen,
            underrun_observed: this.underrun_seen,
        }
    }

    /// Wait without busy-spinning. Cancelling this future does not stop motion.
    pub async fn wait_idle(mut self: Pin<&mut Self>) {
        loop {
            if self.as_mut().status().state == RunState::Idle {
                return;
            }
            Timer::after_millis(1).await;
        }
    }

    /// Stop both DMA channels before stopping and resetting the PIO state
    /// machine. Any in-flight or queued work is discarded and can be reclaimed
    /// only after this method has quiesced both channels.
    pub fn abort(self: Pin<&mut Self>) -> AbortReport {
        // SAFETY: no command storage moves until channels are quiescent.
        let this = unsafe { self.get_unchecked_mut() };
        if this.state == RunState::Aborted {
            return AbortReport {
                position_uncertain: this.position_uncertain(),
            };
        }
        let uncertain = true;
        this.quiesce_dma();
        this.pio.stop();
        let stalled = this.pio.take_stall_status();
        this.stall_seen |= stalled;
        if stalled && this.state == RunState::Running && !this.sealed {
            this.underrun_seen = true;
        }
        this.pio.reset_to_pull();
        this.drain_candidate = None;
        for slot in &mut this.slots {
            if slot.state != SlotState::Free {
                slot.state = SlotState::DmaDone;
            }
        }
        this.state = RunState::Aborted;
        this.position_uncertain = uncertain;
        AbortReport {
            position_uncertain: uncertain,
        }
    }

    /// Start a clean stream after sealed completion or a completed abort.
    pub fn reset(self: Pin<&mut Self>) -> Result<(), QueueError> {
        // SAFETY: reset is allowed only after all DMA-owned buffers have been
        // reclaimed, so no pointer into pinned storage remains live.
        let this = unsafe { self.get_unchecked_mut() };
        if !matches!(this.state, RunState::Idle | RunState::Aborted)
            || this.slots.iter().any(|slot| slot.state != SlotState::Free)
        {
            return Err(QueueError::InvalidState);
        }
        this.quiesce_dma();
        this.pio.reset_to_pull();
        this.order = [None, None];
        this.order_len = 0;
        this.next_channel = 0;
        this.sealed = false;
        this.submitted_any = false;
        this.stall_seen = false;
        this.underrun_seen = false;
        this.drain_candidate = None;
        this.position_uncertain = false;
        this.state = RunState::Prepared;
        Ok(())
    }

    fn configure_slot(&mut self, slot_index: usize, chain_to: u8, trigger: bool) {
        let slot = &self.slots[slot_index];
        let channel_index = slot.channel;
        let buffer = slot.buffer.as_ref().expect("queued slot has buffer");
        let read_addr = buffer.words.as_ptr() as *const u32;
        let len = N as u32;
        let (write_addr, dreq) = self.pio.dma_endpoint();
        let regs = self.dma.channel(channel_index).regs();

        // `self` is pinned, the slot owns this immutable buffer, and
        // the slot cannot be reclaimed or replaced until count reaches zero
        // (or abort waits for BUSY to clear). FIFO is the PIO TX register.
        regs.read_addr().write_value(read_addr as u32);
        regs.write_addr().write_value(write_addr as u32);
        regs.trans_count().write(|w| {
            w.set_mode(0.into());
            w.set_count(len);
        });
        compiler_fence(Ordering::SeqCst);
        cortex_m::asm::dmb();
        if trigger {
            regs.ctrl_trig().write(|w| {
                w.set_treq_sel(dreq);
                w.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
                w.set_incr_read(true);
                w.set_incr_write(false);
                w.set_chain_to(chain_to);
                w.set_bswap(false);
                w.set_en(true);
            });
        } else {
            // AL1_CTRL is a non-triggering control alias. The active channel
            // will trigger this already-armed successor when its count ends.
            let mut control = pac::dma::regs::CtrlTrig(0);
            control.set_treq_sel(dreq);
            control.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
            control.set_incr_read(true);
            control.set_incr_write(false);
            control.set_chain_to(chain_to);
            control.set_bswap(false);
            control.set_en(true);
            regs.al1_ctrl().write_value(control.0);
        }
        compiler_fence(Ordering::SeqCst);
    }

    fn set_chain_target(&self, channel_index: usize, target: u8) {
        // AL1_CTRL does not trigger a transfer. Updating only CHAIN_TO while
        // the channel is active affects its completion edge, while preserving
        // its current DREQ, data size, and address controls.
        let regs = self.dma.channel(channel_index).regs();
        let mut control = regs.ctrl_trig().read();
        control.set_chain_to(target);
        regs.al1_ctrl().write_value(control.0);
        compiler_fence(Ordering::SeqCst);
    }

    fn trigger_if_unstarted(&mut self, slot_index: usize, predecessor_channel: usize) {
        let slot = &self.slots[slot_index];
        let channel_index = slot.channel;
        let regs = self.dma.channel(channel_index).regs();
        // CHAIN_TO is emitted as part of the predecessor's completion. Read
        // predecessor BUSY first, then successor BUSY, then remaining COUNT.
        // If an N=1 chain transfer starts and finishes between the latter
        // reads, COUNT is already zero and we do not reload its source.
        // The RP2350 contract emits CHAIN_TO as part of channel completion;
        // observing BUSY=false therefore means there is no later completion
        // edge for this generation. This is why the predecessor is sampled
        // before checking the successor's BUSY/remaining count.
        let predecessor_busy = self.channel_busy(predecessor_channel);
        let successor_busy = regs.ctrl_trig().read().busy();
        let remaining = regs.trans_count().read().count();
        if should_manual_trigger(predecessor_busy, successor_busy, remaining, N as u32) {
            // The slot was prepared through the non-triggering AL1 alias.
            // Writing CTRL_TRIG with the same fields starts its transfer.
            self.write_trigger(slot_index, CHANNELS[channel_index]);
        }
    }

    fn trigger_predecessor_if_unstarted(&mut self, slot_index: usize, chain_to: u8) {
        let channel_index = self.slots[slot_index].channel;
        let regs = self.dma.channel(channel_index).regs();
        let busy = regs.ctrl_trig().read().busy();
        let remaining = regs.trans_count().read().count();
        if !busy && remaining == N as u32 {
            self.write_trigger(slot_index, chain_to);
        }
    }

    fn write_trigger(&mut self, slot_index: usize, chain_to: u8) {
        let channel_index = self.slots[slot_index].channel;
        let (_, dreq) = self.pio.dma_endpoint();
        compiler_fence(Ordering::SeqCst);
        cortex_m::asm::dmb();
        self.dma
            .channel(channel_index)
            .regs()
            .ctrl_trig()
            .write(|w| {
                w.set_treq_sel(dreq);
                w.set_data_size(pac::dma::vals::DataSize::SIZE_WORD);
                w.set_incr_read(true);
                w.set_incr_write(false);
                w.set_chain_to(chain_to);
                w.set_bswap(false);
                w.set_en(true);
            });
    }

    fn refresh_dma(&mut self) {
        for index in 0..2 {
            if matches!(
                self.slots[index].state,
                SlotState::Active | SlotState::Armed
            ) {
                let channel = self.slots[index].channel;
                let regs = self.dma.channel(channel).regs();
                let busy = regs.ctrl_trig().read().busy();
                let remaining = regs.trans_count().read().count();
                if remaining == 0 && !busy {
                    compiler_fence(Ordering::Acquire);
                    // Prevent the opposite channel's chain from replaying this
                    // completed source before the slot is reclaimed/reused.
                    regs.ctrl_trig().modify(|w| w.set_en(false));
                    self.slots[index].state = SlotState::DmaDone;
                } else if busy {
                    self.slots[index].state = SlotState::Active;
                }
            }
        }
    }

    fn channel_busy(&self, channel: usize) -> bool {
        self.dma.channel(channel).regs().ctrl_trig().read().busy()
    }

    fn dma_quiescent(&self) -> bool {
        self.slots
            .iter()
            .all(|slot| matches!(slot.state, SlotState::Free | SlotState::DmaDone))
            && !self.channel_busy(0)
            && !self.channel_busy(1)
    }

    fn remove_order(&mut self, index: usize) {
        for i in index..self.order_len - 1 {
            self.order[i] = self.order[i + 1];
        }
        self.order_len -= 1;
        self.order[self.order_len] = None;
    }

    fn quiesce_dma(&mut self) {
        // RP2350 erratum workaround: disable both ends of the chain before
        // issuing CHAN_ABORT, so an aborting channel cannot retrigger its peer.
        for channel in &self.dma.channels {
            channel.regs().ctrl_trig().modify(|w| w.set_en(false));
        }
        compiler_fence(Ordering::SeqCst);
        pac::DMA
            .chan_abort()
            .modify(|w| w.set_chan_abort((1 << CHANNELS[0]) | (1 << CHANNELS[1])));
        while self.channel_busy(0) || self.channel_busy(1) {}
        compiler_fence(Ordering::Acquire);
    }

    fn position_uncertain(&self) -> bool {
        self.position_uncertain
    }
}

impl<'d, PIO: pio::Instance, const SM: usize, const N: usize> Drop for Stepper<'d, PIO, SM, N> {
    fn drop(&mut self) {
        // The runtime owns the channels and its pinned source buffers. Drop
        // blocks until no DMA read can outlive those buffers, then forces STEP
        // and DIR low before PIO resources are dropped.
        self.quiesce_dma();
        self.pio.reset_to_pull();
    }
}

// The PIO bound is used by the public generic type but is imported through the
// backend module in the public API.
use embassy_rp::pio;

#[inline]
fn should_manual_trigger(
    predecessor_busy: bool,
    successor_busy: bool,
    remaining: u32,
    initial_count: u32,
) -> bool {
    !predecessor_busy && !successor_busy && remaining == initial_count
}

enum DrainAction {
    StartCandidate,
    Wait,
    Confirm,
}

fn drain_action(elapsed_us: Option<u64>) -> DrainAction {
    match elapsed_us {
        None => DrainAction::StartCandidate,
        Some(elapsed) if elapsed >= PULL_CONFIRM_US => DrainAction::Confirm,
        Some(_) => DrainAction::Wait,
    }
}

#[cfg(test)]
mod tests {
    use super::{DrainAction, drain_action, should_manual_trigger};

    #[test]
    fn never_retriggers_a_one_word_chain_that_finished_between_status_reads() {
        // BUSY is sampled before COUNT. If the chained N=1 transfer completes
        // before COUNT is read, zero remaining words suppresses a CPU retrigger.
        assert!(!should_manual_trigger(false, false, 0, 1));
        assert!(!should_manual_trigger(false, true, 1, 1));
        assert!(!should_manual_trigger(true, false, 1, 1));
        assert!(should_manual_trigger(false, false, 1, 1));
    }

    #[test]
    fn drain_confirmation_waits_past_the_final_nop_pull_boundary() {
        // First PC0/empty observation may be the final-NOP window. A rapid
        // second poll still cannot declare Idle; one confirmed interval can.
        assert!(matches!(drain_action(None), DrainAction::StartCandidate));
        assert!(matches!(drain_action(Some(0)), DrainAction::Wait));
        assert!(matches!(drain_action(Some(9)), DrainAction::Wait));
        assert!(matches!(drain_action(Some(10)), DrainAction::Confirm));
    }
}
