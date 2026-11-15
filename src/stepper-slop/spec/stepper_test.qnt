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
