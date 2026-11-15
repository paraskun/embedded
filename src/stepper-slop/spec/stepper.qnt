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
