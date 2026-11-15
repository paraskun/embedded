# RP2350 → TMC2209 STEP/DIR protocol implementation plan

Status: all five agent assignments have produced their implementation. The release firmware builds with `nix develop -c cargo xtask build stepper --part rp` for `thumbv8m.main-none-eabihf`, including the PIO assembly macro and real dependencies. The literate specification, scenario checks, sampled safety checks, and independent PIO source trace are available. Independent compiled PIO-word comparison, official `lmt` extraction, and hardware acceptance remain open. See the implementation handoff below for the final interfaces and validation limits.

## 1. Scope and hardware contract

- Drive one TMC2209-LA per PIO state machine (SM), with one DMA-fed TX FIFO per motor. PIO blocks contain multiple SMs; a block itself is not the command endpoint. The initial implementation uses PIO0/SM0 and DMA_CH0.
- The TMC2209 drives the bipolar motor windings; RP2350 supplies STEP and DIR.
- Initial pin assignment: GPIO6 = STEP, GPIO7 = DIR. Verify wiring before enabling the driver.
- Use rising-edge stepping only (`CHOPCONF.dedge = 0`) and external STEP/DIR operation (`VACTUAL = 0`). Fix and document the external microstep resolution before motion. UART configuration is outside this first implementation; prerequisites may be established separately.
- Keep the driver enabled during hold and FIFO starvation. Hold means no STEP pulses, not driver disable or a guarantee of unchanged coil current: TMC2209 standstill/current settings still apply. ENN and current configuration are separate from the word format.
- Use compatible 3.3 V logic, common ground, and a correctly configured/current-limited driver. Initialize STEP low and DIR low before enabling the SM.

## 2. Fixed command format

One native 32-bit DMA transfer is one command, displayed MSB first:

```text
31            30             29                         0
+-------------+--------------+--------------------------+
| step / hold | direction    | delay                    |
+-------------+--------------+--------------------------+
```

- Bit 31: `1` = generate exactly one STEP rising edge; `0` = hold, with STEP low throughout.
- Bit 30: actual DIR pin level (`0` or `1`), applied for both step and hold. Clockwise/counterclockwise depends on motor wiring and is a higher-level mapping.
- Bits 29:0: unsigned duration `D`, in units of **10 µs**.
- Valid range: `1..=0x3fff_ffff`. Zero is reserved and rejected by the Rust encoder; never queue it. Raw/unvalidated words are outside the API contract (zero would underflow the proposed PIO counter).
- Encoding: `(step as u32) << 31 | (direction as u32) << 30 | D`.
- Examples: `step, DIR=0, D=1 → 0x8000_0001`; `step, DIR=1, D=1 → 0xc000_0001`; `hold, DIR=1, D=1 → 0x4000_0001`.
- Configure OSR shifting **left**, explicit blocking pulls, no autopull. Consume fields in order: `out x, 1`, `out pins, 1`, `out y, 30`. OUT pin mapping contains DIR only; mandatory one-bit side-set controls STEP only.

## 3. Timing semantics and achievable rate

`D` is the **whole command slot**, including pull, decode, direction setup, pulse, branches, and waiting. It is not an extra wait appended to those instructions.

With a continuously supplied FIFO:

- Slot duration is exactly `D × 10 µs` at the target clock.
- Each step rises 2.5 µs after its slot starts and stays high for 1 µs.
- Consecutive step commands have rising-edge spacing equal to the preceding command's slot duration, even when direction changes. A hold consumes its complete slot without a pulse.
- `D=1` therefore permits continuous **100,000 external STEP pulses/s**. STEP low time is at least 9 µs, DIR setup before the rising edge is 1.5 µs, and DIR hold after the preceding rising edge is at least 8.5 µs.
- Maximum single-command duration is `10,737.41823 s` (about 2 h 59 min). Longer holds require multiple commands.

This fixes an electrical scheduling target. It cannot guarantee that an unspecified motor/load follows 100 kHz without missed steps. For a 200-full-step/revolution motor at 16 external microsteps/full step, 100 kHz corresponds to 1,875 rpm; at 256 microsteps it is about 117 rpm. Interpolation does not change the configured number of external steps per revolution. Mechanical acceptance requires motor, supply, current, microsteps, load, and an acceleration profile. Do not start or reverse a loaded motor abruptly at this rate.

The TMC2209 timing table specifies STEP high/low minima using `max(tFILTSD, tCLK + 20 ns)` (100 ns typical) and DIR setup/hold minima of 20 ns. The proposed timings leave substantial margin with the normal driver clock. Validate against the clock actually used by the driver.

## 4. PIO clock and cycle budget

Target **2 MHz SM clock**, so one PIO cycle is 0.5 µs and one delay unit is 20 cycles.

```text
divider = clk_sys_freq() / 2_000_000
delay_unit_actual = 20 × divider / clk_sys_freq()
```

- At 150 MHz system clock, divider = **75.0**; at 125 MHz, **62.5** (exact average with fractional-divider cycle jitter).
- Read the configured system clock; do not assume its frequency. Prefer an integer divider for uniform individual cycle lengths. Fix the system clock during motion.
- For this fixed-timing implementation, require exact representability in the PIO 16.8 divider: `(f_sys × 256) % 2_000_000 == 0`, with the quotient in the ordinary legal range `256..=0x00ff_ffff`. Reject unsupported clocks instead of silently clamping or changing the unit. Use the HAL's U24F8 container with the hardware's narrower divider range enforced.
- A fractional divider gives exact average timing, with edges quantized to system-clock cycles; include that bounded jitter and oscillator tolerance in measurements.
- Replace [calculate_divider](rp/src/main.rs) and the current duration-to-cycle encoding. The present divider calculation treats the requested duration as one PIO cycle and clamps large values; it does not account for a complete command slot.

The following **design sketch**, not yet assembled firmware, demonstrates a constant cycle budget. All instructions specify mandatory `.side_set 1`; its remaining four delay bits allow `[0..15]`.

```pio
.program stepper
.side_set 1
.wrap_target
    pull block                  side 0 ; cycle 0 if data available
    out x, 1                    side 0 ; cycle 1: step/hold
    out pins, 1                 side 0 ; cycle 2: DIR
    out y, 30                   side 0 ; cycle 3: D
    jmp !x, hold                side 0 ; cycle 4
    nop                         side 1 [1] ; cycles 5..6: STEP high
    jmp settle                  side 0 ; cycle 7: STEP low
hold:
    nop                         side 0 [1] ; cycles 5..6
    nop                         side 0 ; cycle 7
settle:
    jmp y--, wait_units         side 0 [6] ; cycles 8..14: Y = D-1
wait_units:
    jmp !y, done                side 0 [3] ; 4 cycles, including terminal test
    jmp y--, wait_units         side 0 [15] ; 16 cycles per extra unit
done:
    nop                         side 0 ; final cycle
.wrap
```

For `D >= 1`, cycles per slot are:

```text
pull/decode/branch/pulse = 8 (same for step and hold)
initial decrement       = 7
extra-unit loop          = 20 × (D - 1)
terminal zero test       = 4
final nop                = 1
total                   = 20 × D
```

`jmp y--` tests the old value and decrements unconditionally. The initial decrement converts `D` into `D-1`; the zero test prevents a terminal underflow in the repeat loop. Hardware wrap adds no cycle. No branch-path-dependent timing or CPU compensation is allowed. The program occupies 13 instruction words and can be shared by multiple SMs within a block.

## 5. DMA, starvation, completion, and stopping

- Configure TX-only FIFO joining, 32-bit memory-to-PIO transfers, incrementing source address, fixed FIFO destination, and the selected SM's TX DREQ. Keep command buffers alive and immutable until DMA completion.
- Prime the FIFO before enabling the SM. Use chained/double-buffered DMA for continuous sequences; the existing CPU loop that awaits one transfer before submitting the next is not a continuity guarantee.
- DMA handles transport; the SM owns timing. At `D=1`, consumption is 400 kB/s per motor. Eight queued FIFO words cover at most 80 µs at this rate, excluding the active command. Size refill margins and test under bus contention.
- If the FIFO empties, stall at `pull block` with STEP low and DIR unchanged. Never repeat a stale command. Starvation extends the interval; exact timing is guaranteed only while words are available. Record/report TX stall status outside the PIO path.
- DMA completion means words reached the FIFO, not that motion finished. Define queue-drained completion using DMA completion plus TX FIFO empty plus confirmation that the SM reached the blocking pull; do not infer it from FIFO empty alone.
- A hold can last hours; FIFO starvation is not an immediate stop mechanism. Provide a separate abort that stops DMA and the SM, forces STEP low, clears queued commands, and resets the execution state before restart. Disabling ENN is a separate policy. An abort during STEP high can truncate a pulse, so final position is uncertain.

## 6. Implementation sequence

Before firmware implementation, Agent A drafts the Quint state/types and scope for review, then creates and checks the executable protocol specification. Its agreed properties become acceptance criteria for B–E.

1. Add a small command/timing API in the RP crate: explicit step/hold and DIR types, checked 30-bit delay, constants for 10 µs/20 cycles/2 MHz, exact divider validation, and encoding examples. Optional Duration conversion rounds **up** to whole units and rejects zero/overflow; never wrap or silently clamp. Exact scheduling should use integer units.
2. Replace [stepper.pio](rp/pio/stepper.pio) with the balanced STEP/DIR program and cycle annotations. Assemble it and independently verify the sketch's cycle and pin behavior.
3. Update [main.rs](rp/src/main.rs) for the two pin mappings, left shift, divider, safe initialization, initial FIFO priming, and DMA submission. Start with conservative motion and explicit hold commands; remove the immediate fast direction reversal demo.
4. Add DMA continuity, queue-drained completion, starvation reporting, and abort/restart handling. Keep their behavior separate from the fixed 32-bit format.
5. Validate host-side encoding/divider arithmetic and PIO timing, build for `thumbv8m.main-none-eabihf`, then measure STEP/DIR on RP2350 before connecting/enabling the motor.
6. Validate physical motion with fixed driver settings and a suitable acceleration/deceleration profile. Establish the motor/load's reliable rate before claiming mechanical operation at `D=1`.

## 7. Acceptance checks

- Encoder: both modes/directions, exact bit placement, `D=1`, maximum D, zero rejection, duration rounding and overflow; divider checks for 150 MHz, a representable fractional case, and invalid clocks.
- PIO trace: all mode/direction transitions, `D=1,2,3`, representative longer delays, and analytical maximum-delay verification. Every valid command takes `20D` cycles; exactly one rising edge for step and none for hold. A maximum hold need not be simulated cycle by cycle.
- Logic analyzer: continuous `D=1` gives 10 µs edge spacing, 1 µs high pulses, and specified DIR margins; alternating D gives corresponding variable spacing. Repeat across DMA buffer boundaries and under contention.
- Empty FIFO: no extra edge, STEP low throughout stall, correct next command after refill. Completion cannot occur while the final command is still executing.
- Abort during pulse, hold, DMA refill, and blocked pull: no spontaneous restart or stale command after reinitialization.
- Physical motion: no missed steps for the agreed motor/load trajectory, including the ramp to `D=1` if mechanically achievable. This remains a hardware-specific acceptance condition pending motor details.

## 8. Agent assignments and draft APIs

The command format and timing in sections 1–7 are the shared contract. Five agent roles have separate file ownership; they need not run concurrently. Agent A establishes the formal contract first. Agent E integrates after B–D publish their interfaces and checks their behavior against A's specification.

The signatures below define behavior and ownership. Hardware resource bundles and HAL lifetimes are schematic: resolve their exact Embassy 0.10 types before implementation, without changing the protocol or timing. All firmware APIs must work in `no_std`, without heap allocation. Do not introduce a new async executor or change dependency versions as part of these tasks.

| Order | Agent | Model | Reasoning effort |
| --- | --- | --- | --- |
| 1 | A — Literate Quint specification | GPT-6.1-Sol (`gpt-6.1-sol`) | Medium |
| 2 | B — Command encoding and timing | GPT-6-Luna (`gpt-6-luna`) | Medium |
| 3 | C — PIO backend | GPT-6-Luna (`gpt-6-luna`) | Medium |
| 4 | D — DMA and lifecycle | GPT-6-Luna (`gpt-6-luna`) | Medium |
| 5 | E — Integration and acceptance | GPT-6-Luna (`gpt-6-luna`) | Medium |

A completes the specification handoff first. B/C may then work in parallel; D depends on their interfaces, and E integrates. Launch each role with its explicit model and Medium reasoning setting; when using sub-agents, use `fork_turns="none"` and supply this plan and the approved spec as context so the model override applies.

### Agent A — Formal protocol specification and verification in Quint

**Model:** GPT-6.1-Sol, Medium reasoning.

**Owns:** new `spec/stepper.md` (canonical literate source), extracted `spec/stepper.qnt` and `spec/stepper_test.qnt`, `spec/Makefile` (extraction/check targets), and `spec/README.md` (verification commands/results), under this stepper directory. Additional concrete-instance/PIO-model modules may be extracted if needed. Does not edit firmware modules. E coordinates any resulting changes to this plan.

**Skills:** follow [quint-modeling](../../.agents/skills/quint-modeling/SKILL.md), using its from-requirements flow, and [quint-lang](../../.agents/skills/quint-lang/SKILL.md) for language and CLI details. Read the installed skills before starting; the authoritative source is this plan, with section references recorded in a coverage map.

**Task:** create an executable formal specification of command interpretation, timed STEP/DIR behavior, and the DMA/PIO lifecycle. Check its properties and publish reproducible verification evidence before firmware integration. Start with one motor/SM, two buffer slots, and an ordered eight-word FIFO. CPU producer, DMA engine, and SM coordinate through shared state; plain Quint is appropriate here, without a distributed-message framework.

**Reader-facing deliverable:** follow the official [Quint Literate Specifications workflow](https://quint.sh/docs/literate). Write a short Markdown protocol description interleaved with executable fragments using fences such as `` ```quint stepper.qnt += `` and `` ```quint stepper_test.qnt += ``. Extract the modules with `lmt`; run Quint against those extracted files. The literate source is the editing authority; do not maintain a separate hand-edited copy of the same model.

The document must stand alone for humans and agents: purpose/prerequisites, 32-bit word layout and examples, valid/invalid inputs, timing/unit/divider rules, step/hold and direction behavior, starvation/completion/abort semantics, assumptions, and named guarantees. Distill sections 1–5 into direct normative statements; omit planning history, agent assignments, repeated explanations, and generic introductions. Keep each explanation close to the corresponding formal definition or property. Put detailed run logs and tool setup in the README. B–E must read this document before implementation; the plan remains the task schedule rather than a second competing specification.

**Format-verification gate:** extract from a clean output directory and confirm every executable fence names its intended output and produces complete, parseable modules. Re-extraction must be deterministic and leave no generated-file drift. Verify both directions of the wire format: `decode(encode(command)) = command` for valid commands and `encode(decode(word)) = word` for valid unsigned 32-bit words. Check bit 31 step/hold, bit 30 direction, the 30-bit delay mask, all four mode/direction combinations, minimum/maximum delays, zero-delay rejection, and rejection of integers outside `0..=0xffff_ffff`. Model Quint's unbounded integers explicitly so invalid values cannot become apparently valid through masking. Include the documented example words as regression tests. These checks are required in addition to lifecycle/timing properties below.

**Scope and proposed state:** separate pure command/clock arithmetic, an abstract protocol state machine, and a detailed PIO timing model. Use cohesive records for runtime, DMA ownership, and SM state. State includes run mode, stream sealing, buffer identities/ownership/read indices, ordered FIFO contents, current command/identity, STEP/DIR levels, execution phase, remaining delay, cycle counter, and observed edge/completion events. Use ghost command identities to distinguish repeated identical words and detect loss/replay; these are not extra bits in the wire format.

Before writing transition logic, present the scope, type sketch, atomic-operation boundaries, and assumptions for explicit user review as required by the modeling skill. In particular, agree how the current draft treats Idle after an unsealed underrun and how resubmission resumes motion. Resolve ambiguities through E rather than silently selecting new runtime semantics. This review belongs to A's future task; no approval is needed merely to add this assignment.

**Draft specification API** (names/signatures, not yet executable Quint):

```text
Types:
  Command = { step: bool, direction: bool, delay: int }
  CommandId, BufferId
  Mode = Prepared | Running | Idle | Aborting | Aborted
  ProtocolState, DmaState, PioState

Pure functions:
  validCommand(command: Command): bool
  encode(command: Command): int
  decode(word: int): Command
  validClock(sysHz: int): bool
  dividerRaw(sysHz: int): int
  canQueue(state: ProtocolState, buffer: BufferId): bool
  applyQueue(state: ProtocolState, buffer: BufferId): ProtocolState
  canReportIdle(state: ProtocolState): bool

Actions:
  init
  queueBuffer(buffer, commands), sealStream, start
  dmaWriteWord, dmaBufferComplete, reclaimBuffer(buffer)
  pioPull, pioCycle
  observeStall, reportIdle
  beginAbort, quiesceDma, stopSmAndFlush, finishAbort, reset
  step                          // nondeterministic enabled-action composition

State predicates:
  commandOrder, noReplay, bufferOwnership, completionSound
  pulseCount, pulseTiming, slotTiming, directionMargins
  starvationSafe, sealedStreamSafe, abortSafe, resetSafe
  safety                        // conjunction of named safety predicates
```

Separate guards from pure updates and use thin guarded actions; do not add blanket no-op transitions to hide disabled actions. Keep scenario tests in the test module and `init`, `step`, and state invariants in the main model. Concrete instance modules must bind every parameter so simulations execute, not merely typecheck.

**Atomicity and time:** model DMA writes and SM consumption as independently interleavable operations. Model abort as multiple phases so pending DMA writes can race with stopping until DMA is quiescent; do not assume the whole abort is atomic. Keep buffer reclamation separate from executed-command completion. Use PIO cycles as logical integer time because cycle bounds are a primary property. CPU/DMA actions must not silently add or erase SM cycles: document their relationship to the clock. Wall-clock oscillator error, analog signals, real DMA latency, Rust aliasing/pinning soundness, and motor mechanics remain separate implementation/hardware obligations.

The abstract contract may state the intended `20D` slot length. The detailed timing model must derive elapsed cycles and edges from instruction/phase transitions, including instruction delays, branches, wrap, and stalled pull. Check its observations against the abstract contract; setting elapsed time directly to `20D` and then asserting that equality is not timing verification. Map the detailed model to C's assembled program, and require E to check that mapping after integration.

**Required properties and scenarios:**

- **High — wire contract (§2):** encode/decode round trip for valid commands, exact field boundaries, zero-delay rejection, and no invalid command admission. Test the real 30-bit maximum with pure arithmetic even when execution scenarios use small delays.
- **High — pulse/timing contract (§3–4):** each normally completed step produces one rising edge, each hold none; pulse offset is 5 cycles, high duration 2 cycles, whole slot `20D`, and DIR setup/hold margins match the plan. Account for a partly executed command separately; abort may truncate a pulse. Consecutive-slot spacing properties are conditional on continuous FIFO supply and no abort.
- **High — ordering and ownership (§5, Agent D):** no command reordering/replay, no reclamation while DMA still reads the buffer, FIFO capacity respected, and exactly accounted queued/active/completed/discarded commands. Abstract ownership does not prove Rust memory safety.
- **High — completion (§5):** no Idle/completion report while DMA can still write, a command is executing, or data remains queued. Buffer reclamation alone is not motion completion. Reach the case where DMA has completed but the last command is still executing.
- **High — starvation/abort/reset (§5):** empty-FIFO pull keeps STEP low and DIR stable, refill resumes with the next command, abort cannot replay discarded work, and reset cannot admit residual FIFO/DMA data. Check abort interleavings during pulse high, a hold, DMA refill, and blocked pull.
- **Medium — lifecycle (Agent D):** sealed streams reject later queue submissions; expected final stall is distinct from underrun; restart and reset guards match the agreed API. Validate clock-divider preconditions with executable tests rather than relying on unenforced `assume` declarations.

Add a reachable witness per major action and explicit scenarios for all mode/direction transitions, `D=1,2,3`, a DMA boundary, hold, underrun/refill, sealed completion, and abort/reset. A witness true at initialization is not action coverage. Treat finite-stream progress as conditional on fair SM/DMA service and no abort; unconstrained nondeterminism permits indefinite starvation. Do not claim liveness from sampled finite traces. Inspect unexpectedly short traces for disabled-action dead ends, distinguishing intended terminal states from accidental deadlock.

**Verification workflow and evidence:** follow the skills' incremental sequence: typecheck the type sketch, then add one operation at a time, typecheck and run after each increment. Run `quint test` for deterministic scenarios and arithmetic boundaries, and `quint run` with the named invariants and witnesses over concrete small instances. Start with small command/delay domains and increase trace depth/sample count; include an eight-entry FIFO instance and both buffer slots. Record exact tool versions, commands, instance parameters, seeds, sample counts, maximum steps, witness reach counts, results, and any counterexample traces in the README. Perform the skill's review checklist before handoff.

Describe passing `quint run` results as **sampled invariant checking**, not exhaustive proof. This task does not silently substitute simulation for a proof claim. Per the modeling skill, exhaustive bounded checking with `quint verify` is a separate step only if the user explicitly requests model checking; document its bounds and limitations if added. No verification result is claimed until the commands actually run.

**Handoff:** concise literate specification, reproducibly extracted runnable modules/tests, passing extraction/format checks, reviewed assumptions/state sketch, requirement-to-property/action coverage map, reproducible checked results, and representative traces usable as independent firmware test vectors. Document abstractions, exclusions, and remaining proof/hardware obligations. B–E must preserve the approved specification; design changes require updating and rechecking the literate source first. If model and implementation disagree, investigate the counterexample rather than weakening the property to match code. No implementation agent starts until this reader-facing specification and its verification evidence are available.

### Agent B — Command encoding and timing configuration

**Model:** GPT-6-Luna, Medium reasoning.

**Owns:** new `rp/src/proto.rs` and its focused tests. Does not edit the PIO, runtime, or application entry point.

**Task:** implement the validated command format, integer duration conversion, and exact divider calculation. Keep the arithmetic independent of hardware clock reads so it can be tested on the host. The application supplies `clk_sys_freq()`.

```rust
pub const DELAY_UNIT_NS: u64 = 10_000;
pub const CYCLES_PER_UNIT: u32 = 20;
pub const SM_CLOCK_HZ: u32 = 2_000_000;
pub const MAX_DELAY_UNITS: u32 = 0x3fff_ffff;

pub enum Action { Hold, Step }
pub enum Direction { Low, High }
pub enum DelayError { Zero, OutOfRange }
pub enum ClockError { DividerOutOfRange, NotExactlyRepresentable }

pub struct DelayUnits(u32); // private field; Copy
impl DelayUnits {
    pub fn new(units: u32) -> Result<Self, DelayError>;
    pub fn from_nanos_ceil(ns: u64) -> Result<Self, DelayError>;
    pub fn get(self) -> u32;
}

#[repr(transparent)]
pub struct Command(u32); // private field; Copy
impl Command {
    pub fn new(action: Action, dir: Direction, delay: DelayUnits) -> Self;
    pub fn word(self) -> u32;
    pub fn try_from_word(word: u32) -> Result<Self, DelayError>;
}

pub struct TimingConfig { /* validated divider, private fields; Copy */ }
impl TimingConfig {
    pub fn for_sys_clock(sys_hz: u32) -> Result<Self, ClockError>;
    pub fn divider(self) -> fixed::types::U24F8;
}
```

`from_nanos_ceil` rejects zero and rounds positive durations upward using overflow-safe arithmetic. No coupling to Embassy timer resolution is required. `TimingConfig` can only represent the fixed 2 MHz target; do not expose a constructor accepting an arbitrary divider. `Command` has no unchecked public constructor or mutable word accessor.

**Handoff:** API plus encoding/clock test results, including boundary values. C consumes `TimingConfig`; D and E consume `Command`. Deliver the constants first so other agents do not duplicate them.

### Agent C — PIO program and hardware backend

**Model:** GPT-6-Luna, Medium reasoning.

**Owns:** [stepper.pio](rp/pio/stepper.pio) and new `rp/src/stepper.rs`, with PIO assembly/trace checks. Does not manage DMA buffers or edit the application entry point.

**Task:** assemble and verify the 13-word sketch, load it, configure STEP side-set and DIR OUT mappings, and provide a disabled, initialized SM to Agent D. Use the same loaded program for multiple SMs where practical; keep the first integration on PIO0/SM0.

```rust
pub struct PioStepper<'d, P, const SM: usize> { /* owns SM and pins */ }
pub struct PioResources<'d, P, const SM: usize> { /* SM, STEP, DIR */ }
pub enum PrepareError { ProgramLoad, PinMapping }

impl<'d, P, const SM: usize> PioStepper<'d, P, SM> {
    pub fn new(
        common: &mut Common<'d, P>,
        resources: PioResources<'d, P, SM>,
        timing: TimingConfig,
    ) -> Result<Self, PrepareError>;

    pub(crate) fn enable(&mut self);
    pub(crate) fn disable_and_force_step_low(&mut self);
    pub(crate) fn reset_to_pull(&mut self); // disabled; clear FIFO/state/stall flag
    pub(crate) fn tx_level(&self) -> u8;
    pub(crate) fn at_blocking_pull(&self) -> bool;
    pub(crate) fn take_tx_stall(&mut self) -> bool;
    // Also expose the selected SM's typed TX DMA endpoint to runtime.rs.
}
```

`new` configures the divider, left shift, explicit pulls, TX-only FIFO join, pin directions, and both initial pin levels, and returns with the SM disabled. Preserve loaded-program ownership/lifetime for as long as the SM can run. The TX endpoint must support hardware chaining; do not reduce it to an awaited single-buffer helper that prevents Agent D's design.

`at_blocking_pull` must establish a current idle condition from the pull address, empty FIFO, and a current stall indication while the SM is enabled and no producer can refill it. A sticky historical TXSTALL bit alone is insufficient. Agent D serializes this observation with submission and abort. If the HAL lacks the needed accessors, implement the register access here rather than exposing registers to the application.

**Handoff:** backend plus assembled instruction count, cycle/pin trace evidence for both paths, and the exact pull address/idle-observation method. Prove all delay fields fit mandatory one-bit side-set. Hardware execution remains unverified until E measures it.

### Agent D — DMA queue and motion lifecycle

**Model:** GPT-6-Luna, Medium reasoning.

**Owns:** new `rp/src/runtime.rs` and queue/lifecycle tests. Does not edit B/C's files or the application entry point. Request backend additions through C.

**Task:** provide a bounded, two-buffer queue with hardware DMA chaining, correct buffer ownership, starvation detection, drained completion, and abort/restart. Reserve two DMA channels for ping-pong transport; update the initial one-channel resource budget from section 1 accordingly. The command format remains unchanged.

```rust
pub struct CommandBuffer<const N: usize> { /* private [u32; N] */ }
impl<const N: usize> CommandBuffer<N> {
    pub fn new(commands: [Command; N]) -> Self;
}

pub struct Submission(u32); // opaque identifier
pub enum QueueError { Full, EmptyBuffer, InvalidState, StreamClosed }
pub enum RunState { Prepared, Running, Idle, Aborted }
pub struct Status {
    pub state: RunState,
    pub tx_stall_observed: bool,
}
pub struct AbortReport { pub position_uncertain: bool }
pub struct Stepper<'d, P, const SM: usize, const N: usize> { /* owns PIO + DMA */ }

impl<'d, P, const SM: usize, const N: usize> Stepper<'d, P, SM, N> {
    pub fn new(
        pio: PioStepper<'d, P, SM>,
        dma: DmaPair<'d>,
    ) -> Self;
    pub fn queue(
        self: core::pin::Pin<&mut Self>, buffer: CommandBuffer<N>,
    ) -> Result<Submission, (QueueError, CommandBuffer<N>)>;
    pub fn finish(self: core::pin::Pin<&mut Self>) -> Result<(), QueueError>;
    pub fn start(self: core::pin::Pin<&mut Self>) -> Result<(), QueueError>;
    pub fn try_reclaim(
        self: core::pin::Pin<&mut Self>,
    ) -> Option<(Submission, CommandBuffer<N>)>;
    pub fn status(self: core::pin::Pin<&mut Self>) -> Status;
    pub async fn wait_idle(self: core::pin::Pin<&mut Self>);
    pub fn abort(self: core::pin::Pin<&mut Self>) -> AbortReport;
    pub fn reset(self: core::pin::Pin<&mut Self>) -> Result<(), QueueError>;
}
```

The fixed capacity is two outstanding buffers, including active and completed-but-unreclaimed buffers. Buffers move into the driver and return only when DMA no longer reads them; reclamation does **not** imply motion completion. Reject `N=0`. Keep DMA storage at stable addresses while transfers run: the driver owns its slots, is `!Unpin`, and requires pinned receivers. Its destruction must synchronously quiesce DMA and force STEP low before its storage becomes invalid. Queue submission must not expose mutable access to active words. No implicit infinite repetition.

`start` primes the FIFO before enabling the SM and fails if no command is available. During Running, `queue` arms the next chain without a CPU gap when submitted before its documented deadline. Hardware chaining guarantees a buffer boundary only when its successor is already armed; running out of queued work may stall. Report the refill deadline and required interrupt/service margin for the selected N at 100 kHz. `finish` seals a nonempty stream in Prepared or Running, rejects subsequent queue submissions with StreamClosed, and declares that the last queued buffer is final. A sealed stream transitions to Idle once execution drains and rejects further queue submissions until reset. An unsealed stream stalling for lack of data remains Running, records underrun, and resumes automatically on refill without `start`. Status must distinguish normal sealed completion from underrun even though both set the hardware TXSTALL flag.

`wait_idle` requires completed DMA, no queued work, an empty TX FIFO, and C's current blocking-pull observation. Completed buffers awaiting reclamation do not prevent execution from being idle. Cancellation of the wait leaves motion running. It must not busy-spin and starve Embassy tasks. Abort first prevents further DMA writes, then stops execution/forces STEP low and discards queued work; release all owned buffers through reclamation after quiescing DMA. Report position uncertainty conservatively. Reset is allowed only from Aborted or Idle after reclaiming all buffers; it clears stale flags/stream sealing and returns to Prepared at pull without replaying data. Interrupt handlers belong here; E binds them in the application.

**Handoff:** stable queue API, DMA/IRQ resource requirements, cancellation rules, continuity evidence, and drain/abort tests. Review the resource and pinning signatures with C and E before implementing hardware transfers.

### Agent E — Integration and independent acceptance checks

**Model:** GPT-6-Luna, Medium reasoning.

**Owns:** [main.rs](rp/src/main.rs), module declarations or a small library entry point if host tests require it, [Cargo.toml](rp/Cargo.toml) only if needed, and new `rp/tests/` verification files plus a hardware validation note. Coordinates interface changes and updates this plan. Does not rewrite B–D's implementations while they work.

**Task:** wire the three modules together, bind the additional DMA IRQ resources, supply the actual system clock, prepare GPIO6/7, allocate stable DMA storage, and demonstrate a conservative finite step/hold sequence. The application must treat TMC2209 setup as an explicit prerequisite. Use the queue's declared end-of-stream operation before waiting for motion completion.

Draft application-facing composition:

```rust
let timing = TimingConfig::for_sys_clock(clk_sys_freq())?;
let pio = PioStepper::new(&mut common, resources, timing)?;
// Allocate/pin driver and buffer slots before any DMA transfer starts.
let mut motor = /* stable allocation of Stepper::new(pio, dma_pair) */;
let delay = DelayUnits::new(100)?; // 1 ms command slots
let commands = CommandBuffer::new([
    Command::new(Action::Step, Direction::Low, delay),
    Command::new(Action::Hold, Direction::Low, delay),
]);
motor.as_mut().queue(commands)?;
motor.as_mut().finish()?; // seal finite stream
motor.as_mut().start()?;
motor.as_mut().wait_idle().await;
```

This example is schematic; handle errors explicitly in the `no_std` entry point. No acceleration planner API is introduced in this phase. E writes independent checks against requirements rather than reusing the production timing arithmetic as the test oracle.

**Handoff:** target build result, encoding and assembled-PIO verification results, integration/lifecycle checks, and a repeatable logic-analyzer procedure. Record actual hardware measurements separately from modeled results. Only claim motor operation after the hardware-specific acceptance in section 7 passes.

### Dependency order and interface freeze

1. A extracts the formal contract from sections 1–7 and the draft APIs, resolves semantic gaps with E, and presents the model scope/types for review before transition logic. A then delivers the initial executable, checked specification. Interface planning may proceed while this happens; firmware implementation follows the agreed formal contract.
2. B publishes the protocol constants/types; C publishes its backend resource and TX endpoint signatures; D/E agree on static storage, DMA pair/IRQs, buffer reclamation, stream sealing, and completion semantics against A's model. These are interface-review prerequisites, not unresolved choices for individual agents to make independently.
3. B and C implement independently. D builds the queue against C's agreed endpoint and B's validated buffers. E prepares independent acceptance checks using A's properties/traces and then integrates the delivered modules. A rechecks the model when an agreed interface change affects its behavior.
4. File owners make changes to their own modules; E owns cross-module wiring. Any timing change must update A's model, the cycle proof, and tests before integration. No agent changes the wire format or 10 µs unit unilaterally.
5. E runs the combined target build and relevant checks once interfaces are connected, including correspondence to A's checked specification. Hardware validation is a separate final gate, including minimum-delay operation and DMA boundaries. Schedule A first and reuse agent capacity for the remaining roles rather than requiring five simultaneous agents.

## 9. Implementation handoff

- [Literate specification](spec/stepper.md) is the protocol authority; [verification evidence](spec/README.md) records 16 passing scenario groups, 10,000 × 300 main sampled safety checking, additional normal-operation witness coverage, and the explicit extraction/proof limits.
- [Command/timing API](rp/src/proto.rs) implements the frozen word format and exact 2 MHz divider. Its initial standalone arithmetic tests used a substitute fixed-point type; the release target build now compiles against the real dependency.
- [PIO backend](rp/src/stepper.rs) uses `PioResources::new(common, sm, step_pin, dir_pin, timing)` with a `PioStepper` alias. It exposes a crate-private DMA endpoint through source-verified `StateMachine::tx_fifo_ptr()` and `tx_treq()` APIs. Stop forces STEP low; reset and drop also force DIR low.
- [DMA runtime](rp/src/runtime.rs) uses a safe typed `DmaPair::new(DMA_CH0, DMA_CH1, irq0, irq1)`. All buffer/lifecycle operations use pinned receivers. `Status` distinguishes sticky TX-stall history from sticky unsealed underrun history. Sealing remains valid after buffers have been reclaimed. Completion requires producer quiescence and two empty-pull snapshots separated by at least 10 µs; this delays reporting, not command execution. `wait_idle` polls asynchronously. Abort conservatively reports position uncertainty.
- [Demo](rp/src/main.rs) prequeues two buffers containing one step and three holds, all with 1 ms slots and DIR low, then seals, starts, and waits for execution to drain. Both DMA handlers use DMA_IRQ_0. The dependency enables `unstable-pac` for hardware chaining; the workspace excludes the non-Rust specification directory.
- [Independent source-level check](rp/tests/check_pio_trace.py) interprets the actual PIO source, including OSR decoding, and matches the Quint trace. This is not a compiled-assembler comparison. [Bench acceptance](rp/HARDWARE_VALIDATION.md) remains required before claiming measured electrical or motor performance.

Earlier sandbox-only attempts could not load the complete workspace or fetch dependencies. The approved root Nix-shell xtask build succeeds against the complete workspace; no unrelated project files were restored. The release ELF is `target/thumbv8m.main-none-eabihf/release/stepper-rp` relative to the repository root. Building assembles the PIO program; independent comparison of the compiled instruction words and physical measurements remain separate acceptance checks.

## References

- [TMC2209 datasheet, Rev. 1.09](https://www.analog.com/media/en/technical-documentation/data-sheets/TMC2209_datasheet_rev1.09.pdf), STEP/DIR timing and driver configuration.
- [RP2350 datasheet](https://datasheets.raspberrypi.com/rp2350/rp2350-datasheet.pdf), PIO instruction semantics, side-set/delay encoding, FIFO, and clock divider.
- Existing [RP firmware](rp/src/main.rs), [PIO program](rp/pio/stepper.pio), and [crate manifest](rp/Cargo.toml).
