# Checked STEP/DIR specification

[stepper.md](./stepper.md) is the editing authority. [stepper.qnt](./stepper.qnt)
and [stepper_test.qnt](./stepper_test.qnt) are generated outputs. Update and check
the specification before changing its firmware contract. The user approved scope,
types, operation boundaries and underrun semantics on 2026-11-15.

## Reproduction

Tools actually used: Quint **0.33.1**, Rust simulation backend; Python **3.14.7**;
GNU Make **4.4.1**. No npm installation or dependency changes are needed. In this
session the usable absolute tool paths are:

```sh
QUINT=/nix/store/y5fbcf7i6sdpmls45yhshyl7frcdjgrl-quint-0.33.1/bin/quint
PYTHON=/nix/store/d64q19q1xjdwfhqx6czvrjgrhq0n3lcc-python3-3.14.7/bin/python3
MAKE=/nix/store/c011mk580m964s1zxjccvdax3fcmppnl-gnumake-4.4.1/bin/make
cd src/stepper/spec
"$MAKE" dev-extract dev-format typecheck QUINT="$QUINT" PYTHON="$PYTHON"
"$MAKE" test QUINT="$QUINT"
"$MAKE" sample QUINT="$QUINT"
"$MAKE" service STEPS=150 QUINT="$QUINT"
```

`dev-extract` uses the explicitly labeled [development extractor](./dev_extract.py).
[check_extract.py](./check_extract.py) checks every executable fence has a named
output, extracts into two clean directories, compares both results, and checks
generated-file drift. Both resulting modules parse/typecheck and execute. This
development check **does not establish the official lmt gate**.

With an already available `lmt`, the required official workflow is:

```sh
make extract format typecheck test sample service
```

`format` runs `lmt` in two independent clean directories, checks the exact output
set and deterministic bytes, and compares generated outputs. This official target
has not been run because `lmt` is unavailable. The attempted installation was:

```sh
env GOBIN=/tmp/stepper-spec-tools/bin GOPATH=/tmp/stepper-spec-tools/go \
  GOCACHE=/tmp/stepper-spec-tools/cache \
  /nix/store/1fng2iqlx5y8rqvhjp9203mymzyny6cz-go-1.27.3/bin/go \
  install github.com/driusan/lmt@latest
```

The sandbox returned `proxyconnect tcp: dial tcp 127.0.0.1:10808: socket:
operation not permitted`. The required escalated retry returned
`Rejected("rejected by user")`. No equivalent installation retry or network
workaround was attempted. This is a tooling limitation, separate from protocol
property results. The session also makes `/tmp` read-only to these commands;
development extraction uses temporary directories inside this specification
directory instead.

## Results recorded on 2026-11-15

These are **sampled invariant checks and executable scenarios**, not exhaustive
proof or liveness verification. No `quint verify` command was run.

| Check | Result |
|---|---|
| Type-only reviewed sketch, then queue/transport/PIO/abort increments | Typechecked and executed after each layer |
| Final generated modules | Quint typecheck passed |
| Development format check | Two clean extractions, deterministic repeat, no generated drift |
| `quint test stepper_test.qnt --max-samples 10000 --seed 1 --verbosity 1` | **16 passing**, 6,371 ms; timing and full-range wire tests each ran 10,000 samples |
| Main `step`, `safety`, 10,000 traces × at most 300 transitions, seed `1` | **No violation found**, 156,458 ms |
| `serviceStep`, `safety`, 10,000 traces × at most 150 transitions, seed `1` | **No violation found**, 82,279 ms |
| Targeted `serviceStep` stall coverage, 10,000 traces × at most 50 transitions, seed `1` | **No violation found**, 25,767 ms |
| Three-command trace, 120 PIO transitions, seed `1` | **No violation found**, 158 ms |
| Official lmt extraction/format gate | **Not run: tool unavailable; installation rejected** |

Exact main run (the same arguments are in `make sample`):

```sh
quint run stepper.qnt --invariant safety \
  --witnesses queued dmaWrote dmaCompleted reclaimed sealed started pulled cycled \
  stalled stallTicked idled abortBegan dmaQuiesced smStopped abortFinished \
  resetHappened dmaDoneMotionActive pulseHighReached completedCommand \
  --max-samples 10000 --max-steps 300 --seed 1 --verbosity 1
```

The main run reported the following reach counts out of 10,000 traces:

| Witness | Count |
|---|---:|
| queued / dmaWrote / dmaCompleted / reclaimed / sealed | 10,000 each |
| started | 9,982 |
| pulled | 9,944 |
| cycled | 9,782 |
| abortBegan / dmaQuiesced / smStopped / abortFinished / resetHappened | 10,000 each |
| dmaDoneMotionActive | 9,939 |
| pulseHighReached | 2,973 |
| stalled / stallTicked / idled / completedCommand | 0 each |

The zero counts expose abort-heavy sampling: unconstrained abort commonly stops
the SM before a command finishes. They are not presented as action coverage.
The normal-operation driver `serviceStep` excludes abort, preserves all other
guards and transitions, and separately reaches completion:

```sh
quint run stepper.qnt --step serviceStep --invariant safety \
  --witnesses idled completedCommand pulseHighReached \
  --max-samples 10000 --max-steps 150 --seed 1 --verbosity 1
```

This run used the **non-default `--step serviceStep`**. It reached `idled` in
9,907 traces, `completedCommand` in all 10,000, and `pulseHighReached` in 8,932.
The tool printed reproduction seeds `0x38e9577` (main) and `0x1298443` (service);
the invocations above record the supplied seed argument exactly.

The additional targeted stall-coverage command was:

```sh
quint run stepper.qnt --step serviceStep --invariant safety \
  --witnesses stalled stallTicked --max-samples 10000 --max-steps 50 \
  --seed 1 --verbosity 1
```

It also used **non-default `--step serviceStep`**, reached `stalled` in 2,279
traces and `stallTicked` in 3,472, and printed reproduction seed `0x632c17`.
Together the main and normal-operation runs reach every major action's named
witness in more than zero traces. Guard review finds no intended terminal state:
abort permits reclaim/reset and normal blocked pull permits real clock ticks.
The sampled runs used verbosity 1 and do not individually certify every trace's
length; the saved concrete trace reaches its requested 120 PIO cycles.

The deterministic scenarios cover all four mode/direction combinations at D=1,
2,3; all 16 mode/direction pair transitions across two buffers; D=101; analytical
maximum duration; independent wire boundary words and invalid integer domains;
nanosecond rounding/divider boundaries; DMA-done while motion remains active;
capacity-eight FIFO; unsealed underrun/refill; sealed completion/reclaim/reset;
and abort during pulse, hold, refill, and blocked pull. `allTimingCasesTest` and
`allTransitionsTest` each loop over every listed case rather than sample them.

## Independent trace for firmware acceptance

[trace.itf.json](./trace.itf.json) is actual output of the detailed interpreter:

```sh
quint run stepper_test.qnt --main stepper_trace --init traceInit --step traceStep \
  --invariant safety --max-samples 1 --max-steps 120 --seed 1 --verbosity 1 \
  --out-itf trace.itf.json
```

This deliberately uses **non-default `--init traceInit --step traceStep`**. Buffer
0 contains step/low/D1 then hold/high/D2; buffer 1 contains step/high/D3. Expected
observations for comparison with C's independently assembled program are:

| Command identity | Wire word | Pull/start cycle | DIR write | STEP rise | STEP fall | Completion cycle |
|---|---|---:|---:|---:|---:|---:|
| 0 | `0x80000001` | 0 | 2 | 5 | 7 | 20 |
| 1 | `0x40000002` | 20 | 22 | — | — | 60 |
| 2 | `0xc0000003` | 60 | 62 | 65 | 67 | 120 |

At 2 MHz, multiply cycle offsets by 0.5 µs. The trace includes every intervening
cycle and all state, enabling independent inspection rather than only a formula.

## Model boundaries and review

One CPU producer, two owned slots and one SM coordinate via shared state. There
are no distributed messages or injected corrupt commands. The FIFO is ordered
and holds eight words; random actions queue single-word buffers while deterministic
tests additionally queue two-word and nine-word buffers. Identities are ghost
metadata, not wire bits. Each accepted command is accounted for exactly once
between unread transport, FIFO, active execution, completion and discard.

CPU/DMA actions are zero-time orderings between SM ticks. Every SM tick advances
one cycle, including blocked pull. Thus the model checks race/order safety but
does not predict DMA latency, service deadlines or actual bus contention. The
PIO interpreter explicitly executes all 13 addresses, branch paths, side-set,
delay counts and wrap. It decodes from typed fields rather than implementing
hardware OSR bit shifts: arithmetic format checks cover that boundary, and E
must compare actual assembled instruction words/configuration to the interpreter.

The maximum-delay test is analytical: prefix 8 + initial decrement 7 +
`20(D-1)` repeated cycles + terminal test 4 + final nop 1 = `20D`. It does not
simulate 21,474,836,460 cycles. The formula matches the instruction delay table
and both branch paths checked at D=1,2,3,101; this is not an exhaustive proof of
the detailed interpreter for every D.

Self-review against the modeling skill checklist:

| Check | Assessment |
|---|---|
| Thin guarded actions / all variables assigned / maps prepopulated | Pass; one cohesive `state`, pure guard/update functions |
| Typed lifecycle/owner variants and grouped records | Pass; strings only label ghost action coverage |
| Complete `step` composition | Pass; includes every major runtime action |
| Nontrivial invariants | Pass; identity conservation, order/no replay, ownership, sound completion, observed timing |
| Invariants at initialization / after actions | No counterexample observed in executed sampled runs and scenarios |
| Witness coverage | Pass across main and normal drivers: every major action reached; abort-heavy zero counts disclosed |
| Exact format workflow | Warning: development extractor checked; official lmt gate unresolved |
| Proof/physical validation | Not performed; explicitly outside these sampled results |

Remaining obligations: official lmt extraction; independent assembled-code/SM
configuration mapping; Rust ownership/pinning/destruction soundness; actual DMA
chaining/refill margins and cancellation behavior; logic-analyzer measurements;
and motor/load acceptance with fixed TMC settings and acceleration. Fair service
and no abort are required for eventual completion; sampled traces establish no
liveness theorem.
