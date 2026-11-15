# RP2350 STEP/DIR validation

This procedure validates the electrical STEP/DIR output before connecting or
enabling a motor driver. It does not establish that a particular motor/load
can follow the requested rate.

## Bench setup

Use an RP2350 board, a logic analyzer with at least 10 MHz sample rate, and a
common ground. Connect analyzer channels to GPIO6 (STEP) and GPIO7 (DIR). Keep
the TMC2209 disabled or disconnected during the first capture. Use compatible
3.3 V logic. Before any driver test, verify external STEP/DIR mode, rising-edge
stepping (`CHOPCONF.dedge=0`), `VACTUAL=0`, the selected external microstep
resolution, current limit, and wiring. Keep the system clock fixed during the
capture.

## Capture sequence

Use a finite stream with these words, then seal it and wait for the queue's
drained/idle indication:

| Word | Meaning | Expected DIR write | Expected STEP rise | Expected STEP fall | Slot end |
|---|---|---:|---:|---:|---:|
| `0x80000001` | step, DIR low, D=1 | cycle 2 | cycle 5 | cycle 7 | cycle 20 |
| `0x40000002` | hold, DIR high, D=2 | cycle 22 | none | none | cycle 60 |
| `0xc0000003` | step, DIR high, D=3 | cycle 62 | cycle 65 | cycle 67 | cycle 120 |

At 2 MHz, one cycle is 0.5 µs. Relative to the first pull, the STEP rising
edges should therefore occur at 2.5 µs and 32.5 µs; the first pulse should be
1 µs high. The D=2 hold produces no edge, and the final edge follows 30 µs
after the first. The expected observations are also recorded in
[`spec/trace.itf.json`](../spec/trace.itf.json) and the comparison table in
[`spec/README.md`](../spec/README.md).

Repeat with a long hold followed by a step to confirm that a hold consumes its
full slot. Capture a stream boundary with the successor DMA buffer already
armed, and repeat under representative bus/task load. Then allow an unsealed
stream to drain and verify that STEP stays low while blocked; submit another
buffer and verify execution resumes without an explicit restart. Finally test
abort during a pulse, hold, and blocked pull, confirming STEP is forced low and
no queued command replays after reset.

## Acceptance record

Record board revision, measured `clk_sys`, analyzer/sample rate, driver clock,
actual measured STEP high/low times, edge spacing, DIR setup/hold, boundary
continuity, and behavior under load. Compare those measurements to the command
trace; do not infer mechanical position or missed-step performance from logic
levels alone. No hardware measurements are recorded by this software task.

The repository check `python3 src/stepper/rp/tests/check_pio_trace.py` parses
the PIO source, interprets the commands at source level, and compares its
observations with the Quint golden trace and the configured GPIO mapping. It
does not inspect `pio_file!`'s generated instruction words. The release target
build succeeds with `nix develop -c cargo xtask build stepper --part rp`,
including PIO macro assembly and the real dependencies. The ELF is
`target/thumbv8m.main-none-eabihf/release/stepper-rp` relative to the repository
root. Independent compiled instruction-word comparison and hardware
measurements remain pending.
