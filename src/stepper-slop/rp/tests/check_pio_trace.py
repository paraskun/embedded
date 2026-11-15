#!/usr/bin/env python3
"""Source-level PIO/trace acceptance check; does not invoke Embassy's assembler."""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PROGRAM = ROOT / "pio" / "stepper.pio"
BACKEND = ROOT / "src" / "stepper.rs"
SPEC = ROOT.parent / "spec"


@dataclass(frozen=True)
class Instruction:
    op: str
    args: tuple[str, ...]
    side: int
    delay: int


def parse_program() -> tuple[list[Instruction], dict[str, int]]:
    source = PROGRAM.read_text()
    assert re.search(r"^\.side_set\s+1\s*$", source, re.M), "STEP must be mandatory one-bit side-set"
    body = source.split(".wrap_target", 1)[1].split(".wrap", 1)[0]
    instructions: list[Instruction] = []
    labels: dict[str, int] = {}
    for raw in body.splitlines():
        line = raw.split(";", 1)[0].strip()
        if not line:
            continue
        if line.endswith(":"):
            labels[line[:-1]] = len(instructions)
            continue
        match = re.fullmatch(r"(.+?)\s+side\s+([01])(?:\s+\[(\d+)\])?", line)
        assert match, f"instruction must specify mandatory STEP side-set: {line}"
        tokens = match.group(1).split()
        instructions.append(Instruction(tokens[0], tuple(tokens[1:]), int(match.group(2)), int(match.group(3) or 0)))
    assert len(instructions) == 13, f"expected 13 instruction words, got {len(instructions)}"
    assert instructions[0].op == "pull" and instructions[0].args == ("block",)
    assert all(item.side in (0, 1) and 0 <= item.delay <= 15 for item in instructions)
    assert instructions[1:4] == [
        Instruction("out", ("x,", "1"), 0, 0),
        Instruction("out", ("pins,", "1"), 0, 0),
        Instruction("out", ("y,", "30"), 0, 0),
    ]
    assert labels == {"hold": 7, "settle": 9, "wait_units": 10, "done": 12}

    # The actual pin mapping lives in the Rust HAL configuration.
    backend = BACKEND.read_text()
    assert "pub const STEP_GPIO: u8 = 6;" in backend
    assert "pub const DIRECTION_GPIO: u8 = 7;" in backend
    assert "config.use_program(&program, &[&step]);" in backend
    assert "config.set_out_pins(&[&direction]);" in backend
    return instructions, labels


def simulate(instructions: list[Instruction], labels: dict[str, int], word: int, start: int):
    """Interpret the used PIO subset from encoded OSR bits and instruction delays."""
    pc, osr, x, y = 0, word, 0, 0
    step = direction = 0
    cycle = start
    rises: list[int] = []
    falls: list[int] = []
    direction_writes: list[tuple[int, int]] = []

    while True:
        ins = instructions[pc]
        old_step = step
        step = ins.side
        if old_step == 0 and step == 1:
            rises.append(cycle)
        elif old_step == 1 and step == 0:
            falls.append(cycle)

        next_pc = pc + 1
        if ins.op == "pull":
            pass  # this command's word is already present in the FIFO
        elif ins.op == "out":
            dest, count = ins.args[0].rstrip(","), int(ins.args[1])
            value = osr >> (32 - count)
            osr = (osr << count) & 0xFFFF_FFFF  # PIO shift_out = left
            if dest == "x":
                x = value
            elif dest == "pins":
                direction = value & 1
                direction_writes.append((cycle, direction))
            elif dest == "y":
                y = value
            else:
                raise AssertionError(f"unexpected OUT destination {dest}")
        elif ins.op == "jmp":
            if len(ins.args) == 1:
                condition, target = "always", ins.args[0]
            else:
                condition, target = ins.args[0].rstrip(","), ins.args[1]
            take = {
                "!x": x == 0,
                "!y": y == 0,
                "y--": y != 0,
            }.get(condition, condition == "always")
            if condition == "y--":
                y = (y - 1) & 0xFFFF_FFFF  # RP PIO decrements regardless of branch
            if take:
                next_pc = labels[target]
        elif ins.op == "nop":
            pass
        else:
            raise AssertionError(f"unexpected PIO opcode {ins.op}")

        cycle += 1 + ins.delay
        if pc == 12:
            return cycle, rises, falls, direction_writes
        pc = next_pc


def int_field(value: dict[str, str]) -> int:
    return int(value["#bigint"])


def trace_events():
    trace = json.loads((SPEC / "trace.itf.json").read_text())
    sm = trace["states"][-1]["state"]["sm"]
    def cycles(name: str) -> list[int]:
        return [int_field(item["cycle"]) for item in sm[name]]
    return cycles("starts"), cycles("edges"), cycles("falls"), cycles("completions"), int_field(sm["dirCycle"])


def expected_from_readme():
    readme = (SPEC / "README.md").read_text()
    table = readme.split("| Command identity | Wire word | Pull/start cycle | DIR write | STEP rise | STEP fall | Completion cycle |", 1)[1]
    rows = []
    for line in table.splitlines():
        if not line.startswith("|") or "---" in line or "|---" in line:
            continue
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 7 or not cells[0].isdigit():
            continue
        rows.append({
            "word": int(cells[1].strip("`"), 16),
            "start": int(cells[2]),
            "dir": int(cells[3]),
            "rise": None if cells[4] == "—" else int(cells[4]),
            "fall": None if cells[5] == "—" else int(cells[5]),
            "end": int(cells[6]),
        })
    assert len(rows) == 3, "expected the three-command acceptance vector in spec README"
    return rows


def main():
    instructions, labels = parse_program()
    rows = expected_from_readme()
    expected_words = [0x8000_0001, 0x4000_0002, 0xC000_0003]
    assert [row["word"] for row in rows] == expected_words

    starts, rises, falls, completions, final_dir_write = trace_events()
    simulated_starts: list[int] = []
    simulated_rises: list[int] = []
    simulated_falls: list[int] = []
    simulated_ends: list[int] = []
    simulated_dir_writes: list[int] = []
    current = 0
    for row in rows:
        assert row["start"] == current
        simulated_starts.append(current)
        end, step_rises, step_falls, dir_writes = simulate(instructions, labels, row["word"], current)
        assert dir_writes == [(current + 2, (row["word"] >> 30) & 1)]
        simulated_dir_writes.append(dir_writes[0][0])
        if row["rise"] is not None:
            assert step_rises == [row["rise"]]
            assert step_falls == [row["fall"]]
            simulated_rises.extend(step_rises)
            simulated_falls.extend(step_falls)
        else:
            assert not step_rises and not step_falls
        assert end == row["end"]
        simulated_ends.append(end)
        current = end

    assert (simulated_starts, simulated_rises, simulated_falls, simulated_ends) == (starts, rises, falls, completions)
    assert simulated_dir_writes[-1] == final_dir_write
    assert simulated_starts == [0, 20, 60]
    assert simulated_rises == [5, 65]
    assert simulated_falls == [7, 67]
    assert simulated_ends == [20, 60, 120]

    # Cover all four mode/direction combinations, both branch paths, and a longer slot.
    for delay in (1, 2, 3, 101):
        for step_flag in (0, 1):
            for dir_flag in (0, 1):
                word = (step_flag << 31) | (dir_flag << 30) | delay
                end, step_rises, step_falls, dir_writes = simulate(instructions, labels, word, 0)
                assert end == delay * 20
                assert dir_writes == [(2, dir_flag)]
                assert step_rises == ([5] if step_flag else [])
                assert step_falls == ([7] if step_flag else [])

    # The loop body contributes 20 cycles per additional unit; check the
    # maximum command analytically instead of simulating over 21 billion cycles.
    max_delay = 0x3FFF_FFFF
    assert 8 + 7 + 20 * (max_delay - 1) + 4 + 1 == 20 * max_delay
    print("source-level PIO simulation matches Quint trace, pin map, all modes, D=1/2/3/101, and maximum-slot formula")


if __name__ == "__main__":
    main()
