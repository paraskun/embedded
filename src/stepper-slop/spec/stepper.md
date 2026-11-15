# STEP/DIR protocol

One RP2350 state machine drives one TMC2209 STEP/DIR interface. The driver must
already use external STEP/DIR, rising edges, fixed external microsteps, suitable
current and common 3.3 V logic. STEP and DIR initialize low; holding does not
disable the motor driver. Mechanical following, UART configuration, analog timing,
Rust memory safety and measured DMA service margins are outside this model.
In particular, establish `CHOPCONF.dedge=0` and `VACTUAL=0` before motion.

## Representation and scope

One native 32-bit transfer contains step in bit 31, actual DIR level in bit 30,
and unsigned duration `D` in bits 29:0. Valid `D` is `1..0x3fff_ffff`; zero and
integers outside the unsigned 32-bit domain must be rejected before masking.
Examples are `0x8000_0001` (step/low), `0xc000_0001` (step/high), and
`0x4000_0001` (hold/high). Commands own entire slots of `20D` PIO cycles at
2 MHz; the step rises at offset 5 and stays high for 2 cycles. Clock validation
requires an exactly representable legal 16.8 divider.

The executable layers separate pure format/clock arithmetic, the abstract
CPU/DMA/SM lifecycle, and a detailed per-cycle PIO instruction interpreter. The
latter must derive timing from instruction execution rather than assign `20D`.
Both buffer slots and the ordered eight-word FIFO are represented. Ghost command
identities detect replay/loss even when adjacent wire words are equal.

```quint stepper.qnt +=
module stepper {
  type Command = { step: bool, direction: bool, delay: int }
  type CommandId = int
  type BufferId = int
  type Tagged = { id: CommandId, command: Command }
  type Mode = Prepared | Running | Idle | Aborting | Aborted
  type Owner = Free | Queued | Reading | DmaDone
  type Buffer = { owner: Owner, words: List[Tagged], readIndex: int }
  type Active = NoCommand | Current(Tagged)
  type Runtime = {
    mode: Mode, sealed: bool, nextId: int,
    submitted: List[Tagged], pulled: List[Tagged], completed: List[Tagged],
    discarded: List[Tagged], stallObserved: bool, underrun: bool,
    positionUncertain: bool, abortPhase: int,
  }
  type DmaState = {
    slots: BufferId -> Buffer, pending: List[BufferId], quiescent: bool,
  }
  type Observation = { id: CommandId, cycle: int }
  type PioState = {
    enabled: bool, fifo: List[Tagged], current: Active,
    pc: int, instructionDelay: int, x: int, y: int,
    stepPin: bool, dirPin: bool, cycle: int, slotStart: int,
    edges: List[Observation], falls: List[Observation],
    starts: List[Observation], completions: List[Observation],
    dirCycle: int, lastRise: int, marginsOk: bool,
  }
  type ProtocolState = { runtime: Runtime, dma: DmaState, sm: PioState, seen: Set[str] }
  var state: ProtocolState
```

## Operation boundaries and lifecycle

CPU queue, seal, start, reclaim and status observations are serialized atomic
operations. DMA writes one word and SM pulls one word in independent operations;
the FIFO preserves submission order. Buffer DMA completion permits reclamation
but says nothing about completion of its motion. Start requires work and primes
the FIFO before enabling the SM. Sealing rejects further submissions.

**Approved rule:** an unsealed drained stream stays `Running`, reports an
underrun, and leaves the enabled SM blocked at pull. Queueing more work resumes
automatically, without `start`. `Idle` is reserved for sealed drained streams;
the next stream requires all buffers reclaimed and `reset`. A normal final stall
of a sealed stream is distinguished from underrun. Neither a sticky historic
stall flag nor an empty FIFO alone establishes idle: DMA must no longer be able
to write, no commands may remain queued/executing, and the SM must currently be
blocked on its empty pull.

Abort is split into begin, DMA quiescence, SM stop/flush, and finish. DMA writes
and SM cycles may interleave until their respective quiescence/stop operations.
Abort prevents restart, forces STEP low, discards work, permits reclaim only
after DMA quiescence, and reports position uncertainty conservatively. Reset
requires `Idle` or `Aborted` and all buffers reclaimed; it clears stale stream,
FIFO and execution state.

Each SM tick advances one logical PIO cycle, including ticks spent stalled at
pull. CPU/DMA operations are zero-time interleavings between those ticks: this
models ordering races, not CPU/DMA latency. Stalled pull keeps STEP low and DIR
unchanged. No fairness is assumed; finite-stream progress is conditional on fair
DMA/SM service without abort. Validated immutable buffers, reliable ordered
transport and a stable supported system clock are explicit premises.

## Intended guarantees and requirement coverage

| Source | Required check | Priority |
|---|---|---|
| Plan §2 | Both wire round trips, field placement, invalid-domain rejection | High |
| Plan §3–4 | One edge per completed step, no hold edge, derived `20D`, offset 5/high 2, DIR margins | High |
| Plan §5, D | Order/no replay, command conservation, two-slot ownership and FIFO bound | High |
| Plan §5 | DMA completion before motion completion; sound current idle observation | High |
| Plan §5 | Safe stall/refill, multi-phase abort races, clean reset | High |
| Plan D | Sealing, reclaim and approved resume/reset guards | Medium |

This Markdown is canonical and follows the [official literate workflow](https://quint.sh/docs/literate).
The modules were extracted with the documented development tool; official `lmt`
validation remains unrun. Commands, results, bounds and exclusions are in the README.

## Arithmetic and admission

Arithmetic uses unbounded integers. `validWord` checks the input domain before
extracting fields. The divider raw value is the hardware 16.8 integer encoding;
positive nanoseconds round upward. Maximum duration is 10,737.41823 seconds.
Fractional dividers give exact average timing with system-clock quantization;
integer dividers give uniform cycles. The system clock must remain fixed.

```quint stepper.qnt +=
  pure val MAX_DELAY = 1073741823
  pure val WORD_LIMIT = 4294967296
  pure def validCommand(c: Command): bool = c.delay >= 1 and c.delay <= MAX_DELAY
  pure def encode(c: Command): int =
    (if (c.step) 2147483648 else 0) + (if (c.direction) 1073741824 else 0) + c.delay
  pure def decode(w: int): Command = {
    step: w / 2147483648 == 1,
    direction: (w / 1073741824) % 2 == 1,
    delay: w % 1073741824,
  }
  pure def validWord(w: int): bool =
    w >= 0 and w < WORD_LIMIT and validCommand(decode(w))
  pure def dividerRaw(hz: int): int = hz * 256 / 2000000
  pure def validClock(hz: int): bool = and {
    hz > 0, (hz * 256) % 2000000 == 0,
    dividerRaw(hz) >= 256, dividerRaw(hz) <= 16777215,
  }
  pure def nanosCeil(ns: int): int = (ns + 9999) / 10000
  pure def validNanos(ns: int): bool = ns > 0 and nanosCeil(ns) <= MAX_DELAY
  pure val SLOTS = Set(0, 1)
  pure val FIFO_CAPACITY = 8
  pure val COMMANDS = Set(false, true).map(st => Set(false, true).map(dir =>
    Set(1, 2, 3).map(d => {step: st, direction: dir, delay: d})).flatten()).flatten()
  pure val EMPTY_BUFFER: Buffer = {owner: Free, words: List(), readIndex: 0}
  pure val EMPTY_SM: PioState = {
    enabled: false, fifo: List(), current: NoCommand,
    pc: 0, instructionDelay: 0, x: 0, y: 0,
    stepPin: false, dirPin: false, cycle: 0, slotStart: 0,
    edges: List(), falls: List(), starts: List(), completions: List(),
    dirCycle: -100, lastRise: -100, marginsOk: true,
  }
  pure val EMPTY_STATE: ProtocolState = {
    runtime: {mode: Prepared, sealed: false, nextId: 0,
      submitted: List(), pulled: List(), completed: List(), discarded: List(),
      stallObserved: false, underrun: false, positionUncertain: false, abortPhase: 0},
    dma: {slots: SLOTS.mapBy(_ => EMPTY_BUFFER), pending: List(), quiescent: true},
    sm: EMPTY_SM, seen: Set(),
  }
  pure def mark(s: ProtocolState, label: str): ProtocolState = {...s, seen: s.seen.union(Set(label))}
  pure def ids(xs: List[Tagged]): List[int] = xs.foldl(List(), (acc, x) => acc.append(x.id))
  pure def isPrefix(a: List[int], b: List[int]): bool =
    a.length() <= b.length() and a.indices().forall(i => a.nth(i) == b.nth(i))
  pure def activeWords(p: PioState): List[Tagged] = match p.current {
    | NoCommand => List()
    | Current(c) => List(c)
  }
  pure def unread(s: ProtocolState): List[Tagged] = s.dma.pending.foldl(List(), (acc, b) => {
    val slot = s.dma.slots.get(b)
    acc.concat(slot.words.slice(slot.readIndex, slot.words.length()))
  })
  pure def blocked(s: ProtocolState): bool = and {
    s.sm.enabled, s.sm.pc == 0, s.sm.instructionDelay == 0,
    s.sm.fifo.length() == 0, s.sm.current == NoCommand,
  }
  pure def drained(s: ProtocolState): bool = and {
    s.dma.pending.length() == 0, s.dma.quiescent, blocked(s),
  }
  pure def canQueue(s: ProtocolState, b: int, cs: List[Command]): bool = and {
    SLOTS.contains(b), s.dma.slots.get(b).owner == Free,
    Set(Prepared, Running).contains(s.runtime.mode), not(s.runtime.sealed),
    cs.length() > 0, cs.indices().forall(i => validCommand(cs.nth(i))),
  }
  pure def applyQueue(s: ProtocolState, b: int, cs: List[Command]): ProtocolState = {
    val words = range(0, cs.length()).foldl(List(), (acc, i) =>
      acc.append({id: s.runtime.nextId + i, command: cs.nth(i)}))
    mark({...s,
      runtime: {...s.runtime, nextId: s.runtime.nextId + cs.length(),
        submitted: s.runtime.submitted.concat(words)},
      dma: {...s.dma, slots: s.dma.slots.set(b, {owner: Queued, words: words, readIndex: 0}),
        pending: s.dma.pending.append(b), quiescent: false}}, "queue")
  }
  action init = state' = EMPTY_STATE
  action queueBuffer(b: int, cs: List[Command]): bool = all {
    canQueue(state, b, cs), state' = applyQueue(state, b, cs),
  }
```

## Transport and completion

Two slots count outstanding buffers, including DMA-completed buffers awaiting
reclamation. DMA completion releases ownership only after the final source read;
unread words are immutable. A successor can transfer as soon as its predecessor
completes; there is no implicit repeat. The model gives no real-time DMA deadline.
At 100 kHz eight FIFO words provide at most 80 µs excluding the active command;
hardware chaining additionally needs the successor armed before its predecessor
exhausts. Implementation must budget interrupt/service latency separately.

```quint stepper.qnt +=
  pure def canDmaWrite(s: ProtocolState): bool = and {
    not(s.dma.quiescent), s.dma.pending.length() > 0,
    s.sm.fifo.length() < FIFO_CAPACITY,
    s.dma.slots.get(s.dma.pending.head()).readIndex <
      s.dma.slots.get(s.dma.pending.head()).words.length(),
  }
  pure def applyDmaWrite(s: ProtocolState): ProtocolState = {
    val b = s.dma.pending.head()
    val slot = s.dma.slots.get(b)
    mark({...s, dma: {...s.dma, slots: s.dma.slots.set(b,
      {...slot, owner: Reading, readIndex: slot.readIndex + 1})},
      sm: {...s.sm, fifo: s.sm.fifo.append(slot.words.nth(slot.readIndex))}}, "dmaWrite")
  }
  pure def canDmaComplete(s: ProtocolState): bool = and {
    s.dma.pending.length() > 0,
    s.dma.slots.get(s.dma.pending.head()).readIndex ==
      s.dma.slots.get(s.dma.pending.head()).words.length(),
  }
  pure def applyDmaComplete(s: ProtocolState): ProtocolState = {
    val b = s.dma.pending.head()
    val rest = s.dma.pending.tail()
    mark({...s, dma: {...s.dma, pending: rest, quiescent: rest.length() == 0,
      slots: s.dma.slots.setBy(b, slot => {...slot, owner: DmaDone})}}, "dmaComplete")
  }
  pure def canReclaim(s: ProtocolState, b: int): bool =
    SLOTS.contains(b) and s.dma.slots.get(b).owner == DmaDone
  pure def applyReclaim(s: ProtocolState, b: int): ProtocolState =
    mark({...s, dma: {...s.dma, slots: s.dma.slots.set(b, EMPTY_BUFFER)}}, "reclaim")
  pure def canSeal(s: ProtocolState): bool = and {
    Set(Prepared, Running).contains(s.runtime.mode), not(s.runtime.sealed),
    s.runtime.submitted.length() > 0,
  }
  pure def applySeal(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, sealed: true}}, "seal")
  pure def canStart(s: ProtocolState): bool =
    s.runtime.mode == Prepared and s.sm.fifo.length() > 0
  pure def applyStart(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, mode: Running}, sm: {...s.sm, enabled: true}}, "start")
  pure def canObserveStall(s: ProtocolState): bool =
    s.runtime.mode == Running and blocked(s) and not(s.runtime.stallObserved)
  pure def applyObserveStall(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, stallObserved: true,
      underrun: not(s.runtime.sealed)}}, "stall")
  pure def canReportIdle(s: ProtocolState): bool =
    s.runtime.mode == Running and s.runtime.sealed and drained(s)
  pure def applyReportIdle(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, mode: Idle}}, "idle")
  action dmaWriteWord = all {canDmaWrite(state), state' = applyDmaWrite(state)}
  action dmaBufferComplete = all {canDmaComplete(state), state' = applyDmaComplete(state)}
  action reclaimBuffer(b: int): bool = all {canReclaim(state, b), state' = applyReclaim(state, b)}
  action sealStream = all {canSeal(state), state' = applySeal(state)}
  action start = all {canStart(state), state' = applyStart(state)}
  action observeStall = all {canObserveStall(state), state' = applyObserveStall(state)}
  action reportIdle = all {canReportIdle(state), state' = applyReportIdle(state)}
```

## PIO execution

Addresses below correspond in order to the 13-word plan program: pull, out X,
out DIR, out Y, branch, step-high nop, settle jump, hold nop, hold nop,
initial Y decrement, zero test, repeat decrement, final nop. All instructions
side-set STEP low except address 5. Instruction delay counts consume additional
cycles after execution; hardware wrap consumes none. `jmp y--` decrements even
when its old value is zero. The zero test bypasses the terminal underflow.

The interpreter consumes one cycle per transition, including blocked pull ticks.
Command pull, edge and completion timestamps are observations of execution.
The invariant later compares those observations with the abstract `20D` contract.

```quint stepper.qnt +=
  pure def canPull(s: ProtocolState): bool = and {
    s.sm.enabled, s.sm.pc == 0, s.sm.instructionDelay == 0,
    s.sm.current == NoCommand, s.sm.fifo.length() > 0,
  }
  pure def applyPull(s: ProtocolState): ProtocolState = {
    val c = s.sm.fifo.head()
    mark({...s, runtime: {...s.runtime, pulled: s.runtime.pulled.append(c)},
      sm: {...s.sm, fifo: s.sm.fifo.tail(), current: Current(c), pc: 1,
        stepPin: false, slotStart: s.sm.cycle, cycle: s.sm.cycle + 1,
        starts: s.sm.starts.append({id: c.id, cycle: s.sm.cycle})}}, "pull")
  }
  pure def canCycle(s: ProtocolState): bool =
    s.sm.enabled and not(canPull(s))
  pure def nextPc(p: PioState): int =
    if (p.pc == 4) (if (p.x == 0) 7 else 5)
    else if (p.pc == 6) 9
    else if (p.pc == 9) 10
    else if (p.pc == 10) (if (p.y == 0) 12 else 11)
    else if (p.pc == 11) (if (p.y != 0) 10 else 12)
    else if (p.pc == 12) 0 else p.pc + 1
  pure def instructionDelay(pc: int): int =
    if (Set(5, 7).contains(pc)) 1
    else if (pc == 9) 6
    else if (pc == 10) 3
    else if (pc == 11) 15 else 0
  pure def executeInstruction(p: PioState, c: Tagged): PioState = {
    val high = p.pc == 5
    val rise = high and not(p.stepPin)
    val fall = not(high) and p.stepPin
    val dirWrite = p.pc == 2
    {...p, pc: nextPc(p), instructionDelay: instructionDelay(p.pc),
      x: if (p.pc == 1) (if (c.command.step) 1 else 0) else p.x,
      y: if (p.pc == 3) c.command.delay
         else if (Set(9, 11).contains(p.pc)) (p.y + WORD_LIMIT - 1) % WORD_LIMIT else p.y,
      stepPin: high,
      dirPin: if (dirWrite) c.command.direction else p.dirPin,
      dirCycle: if (dirWrite) p.cycle else p.dirCycle,
      lastRise: if (rise) p.cycle else p.lastRise,
      marginsOk: p.marginsOk and
        (not(rise) or p.cycle - p.dirCycle >= 3) and
        (not(dirWrite and p.dirPin != c.command.direction) or p.cycle - p.lastRise >= 17),
      edges: if (rise) p.edges.append({id: c.id, cycle: p.cycle}) else p.edges,
      falls: if (fall) p.falls.append({id: c.id, cycle: p.cycle}) else p.falls,
      cycle: p.cycle + 1,
      current: if (p.pc == 12) NoCommand else p.current,
      completions: if (p.pc == 12) p.completions.append({id: c.id, cycle: p.cycle + 1})
                   else p.completions,
    }
  }
  pure def applyCycle(s: ProtocolState): ProtocolState = {
    val p = s.sm
    if (p.instructionDelay > 0)
      mark({...s, sm: {...p, instructionDelay: p.instructionDelay - 1, cycle: p.cycle + 1}}, "cycle")
    else match p.current {
      | NoCommand => mark({...s, sm: {...p, cycle: p.cycle + 1, stepPin: false}}, "stallTick")
      | Current(c) => mark({...s,
          runtime: if (p.pc == 12) {...s.runtime, completed: s.runtime.completed.append(c)} else s.runtime,
          sm: executeInstruction(p, c)}, "cycle")
    }
  }
  action pioPull = all {canPull(state), state' = applyPull(state)}
  action pioCycle = all {canCycle(state), state' = applyCycle(state)}
```

## Abort and reset

Beginning abort closes submission but leaves DMA and the SM independently live
until stopped. Quiescing DMA accounts for its unread source suffix as discarded;
then SM stop discards FIFO/current commands and forces STEP low. Completed words
remain completed. This models a pending write racing with abort; no write is
possible after quiescence. Abort conservatively reports position uncertainty
even when no pulse was observed. No guarantee is made about a truncated pulse.

```quint stepper.qnt +=
  pure def canBeginAbort(s: ProtocolState): bool =
    Set(Prepared, Running, Idle).contains(s.runtime.mode)
  pure def applyBeginAbort(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, mode: Aborting, abortPhase: 1,
      positionUncertain: true}}, "beginAbort")
  pure def canQuiesce(s: ProtocolState): bool =
    s.runtime.mode == Aborting and s.runtime.abortPhase == 1
  pure def applyQuiesce(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, abortPhase: 2,
      discarded: s.runtime.discarded.concat(unread(s))},
      dma: {...s.dma, pending: List(), quiescent: true,
        slots: SLOTS.mapBy(b => {
          val slot = s.dma.slots.get(b)
          if (slot.owner == Free) slot else {...slot, owner: DmaDone}
        })}}, "quiesce")
  pure def canStop(s: ProtocolState): bool =
    s.runtime.mode == Aborting and s.runtime.abortPhase == 2
  pure def applyStop(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, abortPhase: 3,
      discarded: s.runtime.discarded.concat(s.sm.fifo).concat(activeWords(s.sm))},
      sm: {...s.sm, enabled: false, fifo: List(), current: NoCommand,
        pc: 0, instructionDelay: 0, x: 0, y: 0, stepPin: false}}, "stop")
  pure def canFinishAbort(s: ProtocolState): bool =
    s.runtime.mode == Aborting and s.runtime.abortPhase == 3
  pure def applyFinishAbort(s: ProtocolState): ProtocolState =
    mark({...s, runtime: {...s.runtime, mode: Aborted}}, "finishAbort")
  pure def canReset(s: ProtocolState): bool = and {
    Set(Idle, Aborted).contains(s.runtime.mode),
    SLOTS.forall(b => s.dma.slots.get(b).owner == Free),
  }
  pure def applyReset(s: ProtocolState): ProtocolState =
    mark({...EMPTY_STATE, runtime: {...EMPTY_STATE.runtime, nextId: s.runtime.nextId},
      seen: s.seen}, "reset")
  action beginAbort = all {canBeginAbort(state), state' = applyBeginAbort(state)}
  action quiesceDma = all {canQuiesce(state), state' = applyQuiesce(state)}
  action stopSmAndFlush = all {canStop(state), state' = applyStop(state)}
  action finishAbort = all {canFinishAbort(state), state' = applyFinishAbort(state)}
  action reset = all {canReset(state), state' = applyReset(state)}
```

## Guarantees checked against execution

`commandConservation` partitions each accepted identity between unread DMA, FIFO,
active execution, normal completion, or discard. It also checks uniqueness, so
set equality cannot hide loss plus duplication. `commandOrder` compares ordered
pull/completion logs to accepted order. These ghost logs describe one stream;
reset clears them while retaining monotonically increasing identities.

Timing checks use observed pull/rise/fall/completion cycles. Consecutive-slot edge
spacing holds only when the next pull is at the preceding completion cycle.
The electrical target is 10 µs slots at D=1, 1 µs high, at least 9 µs low,
1.5 µs DIR setup and 8.5 µs DIR hold. No motor/load speed guarantee follows.

```quint stepper.qnt +=
  pure def idSet(xs: List[Tagged]): Set[int] =
    xs.foldl(Set(), (acc, x) => acc.union(Set(x.id)))
  pure def observationTime(xs: List[Observation], id: int): int =
    xs.foldl(-1000000000000, (acc, x) => if (x.id == id) x.cycle else acc)
  pure def observationCount(xs: List[Observation], id: int): int =
    xs.foldl(0, (acc, x) => acc + (if (x.id == id) 1 else 0))
  val commandOrder = and {
    isPrefix(ids(state.runtime.pulled), ids(state.runtime.submitted)),
    isPrefix(ids(state.runtime.completed), ids(state.runtime.pulled)),
  }
  val noReplay = and {
    idSet(state.runtime.pulled).size() == state.runtime.pulled.length(),
    idSet(state.runtime.completed).size() == state.runtime.completed.length(),
    state.sm.edges.indices().forall(i =>
      observationCount(state.sm.edges, state.sm.edges.nth(i).id) == 1),
  }
  val commandConservation = {
    val accounted = state.runtime.completed.concat(state.runtime.discarded)
      .concat(unread(state)).concat(state.sm.fifo).concat(activeWords(state.sm))
    idSet(accounted) == idSet(state.runtime.submitted) and
      accounted.length() == state.runtime.submitted.length() and
      idSet(accounted).size() == accounted.length()
  }
  val bufferOwnership = and {
    state.sm.fifo.length() <= FIFO_CAPACITY,
    state.dma.quiescent == (state.dma.pending.length() == 0),
    SLOTS.forall(b => {
      val slot = state.dma.slots.get(b)
      val pending = state.dma.pending.indices().exists(i => state.dma.pending.nth(i) == b)
      and {
        pending == Set(Queued, Reading).contains(slot.owner),
        slot.readIndex >= 0, slot.readIndex <= slot.words.length(),
        slot.owner != Free or (slot.words.length() == 0 and slot.readIndex == 0),
      }
    }),
  }
  val completionSound = state.runtime.mode != Idle or
    (state.runtime.sealed and drained(state) and
      state.runtime.completed.length() == state.runtime.submitted.length())
  val pulseCount = state.runtime.completed.indices().forall(i => {
    val c = state.runtime.completed.nth(i)
    observationCount(state.sm.edges, c.id) == (if (c.command.step) 1 else 0)
  })
  val pulseTiming = and {
    state.sm.edges.indices().forall(i => {
      val edge = state.sm.edges.nth(i)
      edge.cycle - observationTime(state.sm.starts, edge.id) == 5
    }),
    state.runtime.completed.indices().forall(i => {
      val c = state.runtime.completed.nth(i)
      not(c.command.step) or
        (observationCount(state.sm.falls, c.id) == 1 and
         observationTime(state.sm.falls, c.id) - observationTime(state.sm.edges, c.id) == 2)
    }),
  }
  val slotTiming = state.runtime.completed.indices().forall(i => {
    val c = state.runtime.completed.nth(i)
    observationTime(state.sm.completions, c.id) - observationTime(state.sm.starts, c.id) == 20 * c.command.delay
  })
  val continuousSpacing = state.runtime.completed.indices().forall(i => {
    if (i == 0) true else {
      val prev = state.runtime.completed.nth(i - 1)
      val curr = state.runtime.completed.nth(i)
      val continuous = observationTime(state.sm.starts, curr.id) == observationTime(state.sm.completions, prev.id)
      not(continuous and prev.command.step and curr.command.step) or
        observationTime(state.sm.edges, curr.id) - observationTime(state.sm.edges, prev.id) == 20 * prev.command.delay
    }
  })
  val directionMargins = state.sm.marginsOk
  val starvationSafe = not(blocked(state)) or not(state.sm.stepPin)
  val sealedStreamSafe = state.runtime.mode != Idle or state.runtime.sealed
  val abortSafe = state.runtime.mode != Aborted or and {
    state.dma.quiescent, not(state.sm.enabled), not(state.sm.stepPin),
    state.sm.fifo.length() == 0, state.sm.current == NoCommand,
    state.runtime.positionUncertain,
  }
  val resetSafe = state.runtime.mode != Prepared or and {
    not(state.sm.enabled), not(state.sm.stepPin), not(state.sm.dirPin),
    state.sm.pc == 0, state.sm.current == NoCommand,
  }
  val admissionSafe = state.runtime.submitted.indices().forall(i =>
    validCommand(state.runtime.submitted.nth(i).command))
  val safety = and {commandOrder, noReplay, commandConservation, bufferOwnership,
    completionSound, pulseCount, pulseTiming, slotTiming, continuousSpacing,
    directionMargins, starvationSafe, sealedStreamSafe, abortSafe, resetSafe, admissionSafe}
  val queued = state.seen.contains("queue")
  val dmaWrote = state.seen.contains("dmaWrite")
  val dmaCompleted = state.seen.contains("dmaComplete")
  val reclaimed = state.seen.contains("reclaim")
  val sealed = state.seen.contains("seal")
  val started = state.seen.contains("start")
  val pulled = state.seen.contains("pull")
  val cycled = state.seen.contains("cycle")
  val stalled = state.seen.contains("stall")
  val stallTicked = state.seen.contains("stallTick")
  val idled = state.seen.contains("idle")
  val abortBegan = state.seen.contains("beginAbort")
  val dmaQuiesced = state.seen.contains("quiesce")
  val smStopped = state.seen.contains("stop")
  val abortFinished = state.seen.contains("finishAbort")
  val resetHappened = state.seen.contains("reset")
  val dmaDoneMotionActive = state.dma.quiescent and state.sm.current != NoCommand
  val pulseHighReached = state.sm.stepPin
  val completedCommand = state.runtime.completed.length() > 0
  action serviceStep = {
    nondet b = SLOTS.oneOf()
    nondet c = COMMANDS.oneOf()
    any {queueBuffer(b, List(c)), dmaWriteWord, dmaBufferComplete,
      reclaimBuffer(b), sealStream, start, observeStall, reportIdle, pioPull, pioCycle, reset}
  }
  action step = {
    nondet b = SLOTS.oneOf()
    nondet c = COMMANDS.oneOf()
    any {queueBuffer(b, List(c)), dmaWriteWord, dmaBufferComplete,
      reclaimBuffer(b), sealStream, start, observeStall, reportIdle, pioPull, pioCycle,
      beginAbort, quiesceDma, stopSmAndFlush, finishAbort, reset}
  }
}
```

## Executable regression scenarios

Scenarios use concrete command values and real 30-bit arithmetic boundaries.
Sampled execution domains remain small; this does not bound the encoder's range.

```quint stepper_test.qnt +=
module stepper_test {
  import stepper.* from "./stepper"
  pure val STEP_LOW = {step: true, direction: false, delay: 1}
  pure val HOLD_HIGH = {step: false, direction: true, delay: 1}
  run arithmeticTest = all {
    assert(Set(false, true).forall(st => Set(false, true).forall(dir =>
      Set(1, 2, 3, MAX_DELAY).forall(d => {
        val c = {step: st, direction: dir, delay: d}
        validCommand(c) and decode(encode(c)) == c and encode(decode(encode(c))) == encode(c)
      })))),
    assert(encode(STEP_LOW) == 2147483649),
    assert(encode({...STEP_LOW, direction: true}) == 3221225473),
    assert(encode(HOLD_HIGH) == 1073741825),
    assert(Set(1, 1073741823, 1073741825, 2147483647,
      2147483649, 3221225471, 3221225473, 4294967295).forall(w =>
      validWord(w) and encode(decode(w)) == w)),
    assert(decode(1) == {step: false, direction: false, delay: 1}),
    assert(decode(1073741823) == {step: false, direction: false, delay: MAX_DELAY}),
    assert(decode(1073741825) == HOLD_HIGH),
    assert(decode(2147483647) == {...HOLD_HIGH, delay: MAX_DELAY}),
    assert(decode(2147483649) == STEP_LOW),
    assert(decode(3221225471) == {...STEP_LOW, delay: MAX_DELAY}),
    assert(decode(3221225473) == {...STEP_LOW, direction: true}),
    assert(decode(4294967295) == {step: true, direction: true, delay: MAX_DELAY}),
    assert(Set(-1, 0, 1073741824, 2147483648, 3221225472, WORD_LIMIT, WORD_LIMIT + 1)
      .forall(w => not(validWord(w)))),
    assert(not(validCommand({...STEP_LOW, delay: 0}))),
    assert(not(validCommand({...STEP_LOW, delay: MAX_DELAY + 1}))),
    assert(validWord(WORD_LIMIT - 1)),
    assert(validClock(150000000) and dividerRaw(150000000) == 19200),
    assert(validClock(125000000) and dividerRaw(125000000) == 16000),
    assert(Set(-1, 0, 1999999, 2000001, 1000000000000).forall(hz => not(validClock(hz)))),
    assert(nanosCeil(1) == 1 and nanosCeil(10000) == 1 and nanosCeil(10001) == 2),
    assert(validNanos(MAX_DELAY * 10000) and not(validNanos(MAX_DELAY * 10000 + 1))),
    assert(not(validNanos(0))),
  }
  run timingTest = {
    nondet c = COMMANDS.oneOf()
    init.then(queueBuffer(0, List(c))).then(dmaWriteWord).then(dmaBufferComplete)
      .then(sealStream).then(start).then(pioPull)
      .then((20 * c.delay - 1).reps(_ => pioCycle))
      .expect(state.sm.cycle == 20 * c.delay and state.runtime.completed.length() == 1)
      .then(reportIdle).expect(state.runtime.mode == Idle)
  }
  run dmaBeforeMotionTest = init.then(queueBuffer(0, List(STEP_LOW)))
    .then(dmaWriteWord).then(dmaBufferComplete).then(start).then(pioPull)
    .expect(state.dma.quiescent and state.runtime.completed.length() == 0 and not(canReportIdle(state)))
  run fullRangeWireTest = {
    nondet w = 0.to(WORD_LIMIT - 1).oneOf()
    nondet d = 1.to(MAX_DELAY).oneOf()
    nondet st = Set(false, true).oneOf()
    nondet dir = Set(false, true).oneOf()
    val c = {step: st, direction: dir, delay: d}
    all {
      assert(not(validWord(w)) or encode(decode(w)) == w),
      assert(decode(encode(c)) == c),
    }
  }
  pure def commandCase(i: int, d: int): Command =
    {step: i / 2 == 1, direction: i % 2 == 1, delay: d}
  run allTimingCasesTest = 12.reps(i => {
    val c = commandCase(i / 3, i % 3 + 1)
    init.then(queueBuffer(0, List(c))).then(dmaWriteWord).then(dmaBufferComplete)
      .then(sealStream).then(start).then(pioPull)
      .then((20 * c.delay - 1).reps(_ => pioCycle))
      .then(reportIdle).expect(safety and state.sm.cycle == 20 * c.delay)
  })
  action pairScenario(a: Command, b: Command): bool =
    init.then(queueBuffer(0, List(a))).then(queueBuffer(1, List(b)))
      .then(dmaWriteWord).then(dmaBufferComplete)
      .then(dmaWriteWord).then(dmaBufferComplete).then(sealStream).then(start)
      .then(pioPull).then((20 * a.delay - 1).reps(_ => pioCycle))
      .then(pioPull).then((20 * b.delay - 1).reps(_ => pioCycle))
      .then(reportIdle).expect(safety and state.runtime.completed.length() == 2)
  run allTransitionsTest = 16.reps(i => pairScenario(commandCase(i / 4, 1), commandCase(i % 4, 1)))
  run alternatingDelayTest = pairScenario(commandCase(2, 2), commandCase(3, 3))
  run longDelayTest = init.then(queueBuffer(0, List({...STEP_LOW, delay: 101})))
    .then(dmaWriteWord).then(dmaBufferComplete).then(start).then(pioPull)
    .then(2019.reps(_ => pioCycle)).expect(safety and state.sm.cycle == 2020)
  run maximumAnalyticalTest = all {
    assert(8 + 7 + 20 * (MAX_DELAY - 1) + 4 + 1 == 20 * MAX_DELAY),
    assert(range(0, 13).indices().forall(i => instructionDelay(i) <= 15)),
  }
  run underrunResumeTest = init.then(queueBuffer(0, List(STEP_LOW)))
    .then(dmaWriteWord).then(dmaBufferComplete).then(start).then(pioPull)
    .then(19.reps(_ => pioCycle)).then(observeStall)
    .expect(state.runtime.mode == Running and state.runtime.underrun)
    .then(7.reps(_ => pioCycle)).expect(not(state.sm.stepPin) and not(state.sm.dirPin))
    .then(reclaimBuffer(0)).then(queueBuffer(0, List(HOLD_HIGH)))
    .then(dmaWriteWord).then(dmaBufferComplete).then(sealStream)
    .then(pioPull).then(19.reps(_ => pioCycle)).then(reportIdle)
    .expect(safety and state.runtime.mode == Idle and state.sm.dirPin and
      state.runtime.completed.length() == 2 and state.sm.edges.length() == 1)
  run sealingAndResetTest = init.then(queueBuffer(0, List(STEP_LOW)))
    .then(dmaWriteWord).then(dmaBufferComplete).then(sealStream)
    .expect(not(canQueue(state, 1, List(STEP_LOW))))
    .then(start).then(pioPull).then(19.reps(_ => pioCycle)).then(reportIdle)
    .expect(not(state.runtime.underrun) and not(canReset(state)))
    .then(reclaimBuffer(0)).then(reset).expect(safety and state.runtime.mode == Prepared)
  action finishAbortScenario: bool = beginAbort.then(quiesceDma)
    .then(stopSmAndFlush).then(finishAbort).expect(safety)
  run abortPulseTest = init.then(queueBuffer(0, List(STEP_LOW, STEP_LOW)))
    .then(dmaWriteWord).then(start).then(pioPull).then(5.reps(_ => pioCycle))
    .expect(state.sm.stepPin).then(beginAbort)
    .then(dmaWriteWord).then(pioCycle).then(quiesceDma)
    .then(stopSmAndFlush).then(finishAbort)
    .expect(safety and state.runtime.positionUncertain and state.runtime.discarded.length() == 2)
    .then(reclaimBuffer(0)).then(reset)
    .expect(safety and state.sm.fifo.length() == 0 and state.runtime.nextId == 2)
  run abortHoldTest = init.then(queueBuffer(0, List({...HOLD_HIGH, delay: 3})))
    .then(dmaWriteWord).then(start).then(pioPull).then(12.reps(_ => pioCycle))
    .then(finishAbortScenario).expect(not(state.sm.stepPin))
  run abortRefillTest = init.then(queueBuffer(0, List(STEP_LOW, HOLD_HIGH)))
    .then(queueBuffer(1, List(STEP_LOW))).then(dmaWriteWord)
    .then(beginAbort).then(dmaWriteWord).then(dmaBufferComplete).then(dmaWriteWord)
    .then(quiesceDma).then(stopSmAndFlush).then(finishAbort).expect(safety)
  run abortBlockedPullTest = init.then(queueBuffer(0, List(HOLD_HIGH)))
    .then(dmaWriteWord).then(dmaBufferComplete).then(start).then(pioPull)
    .then(19.reps(_ => pioCycle)).then(observeStall).then(finishAbortScenario)
    .expect(safety and state.runtime.discarded.length() == 0)
  run eightWordFifoTest = init
    .then(queueBuffer(0, range(0, 9).foldl(List(), (acc, _) => acc.append(STEP_LOW))))
    .then(8.reps(_ => dmaWriteWord))
    .expect(state.sm.fifo.length() == 8 and not(canDmaWrite(state)))
    .then(start).then(pioPull).then(dmaWriteWord)
    .expect(state.sm.fifo.length() == 8 and safety)
}
```

For independent firmware comparison, this concrete trace supplies three commands
across two buffers. Its observed timestamps can be exported in ITF form using
the README command. No abort occurs in this trace.

```quint stepper_test.qnt +=
module stepper_trace {
  import stepper.* from "./stepper"
  action traceInit = init
    .then(queueBuffer(0, List({step: true, direction: false, delay: 1},
      {step: false, direction: true, delay: 2})))
    .then(queueBuffer(1, List({step: true, direction: true, delay: 3})))
    .then(dmaWriteWord).then(dmaWriteWord).then(dmaBufferComplete)
    .then(dmaWriteWord).then(dmaBufferComplete).then(sealStream).then(start)
  action traceStep = any {pioPull, pioCycle}
}
```
