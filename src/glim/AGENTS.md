# Agent instructions: glim

## Scope

You may work on infrastructure and supporting code only. **Never change the
firmware.**

Allowed:

* `box/` - the enclosure model (`main.py`) and its browser viewer (`viewer.html`)
* build and dev tooling: `xtask/`, `flake.nix`, `.cargo/config.toml`,
  `.gitignore`, CI, scripts, formatter and linter config
* documentation: this file, `README.md`

Off limits, read-only:

* `rx/` and `tx/`, all of it: sources, `Cargo.toml`, `build.rs`, `memory.x`,
  `pio/`. This includes refactors, formatting, dependency bumps and "obvious"
  fixes.
* `shared/`, when the change would alter firmware behaviour

If a task can't be done without touching firmware, stop. Describe the change
that's needed and where, then leave it for the user. You may read firmware to
understand interfaces (pins, I2C address, frame format) but not to edit it.

Don't flash hardware. The cargo runners and `cargo xtask flash` use `sudo` and
write to devices on the bench. Only flash if the user asks in that session.

## Environment

* Rust toolchain, `espflash`, `picotool` and Python come from the Nix dev shell
  at the repo root (`nix develop`, or direnv via `.envrc`).
* `box/` needs `uv` with Python 3.13 and some required packages (see `requirements.txt` at the root).

## Commands

Run from the repository root:

```sh
cargo xtask list            # projects and parts
cargo xtask check glim      # type-check firmware, a safe way to verify tooling changes
cargo xtask clippy glim
cargo xtask dist glim       # images into target/dist/glim/
cargo xtask test            # host crates
```

Both glim parts are currently listed as `glim/tx`, so `--part` can't tell them
apart. Select the whole project instead.

Run from `box/`:

```sh
python main.py --help
python main.py --step PATH  # quickest full rebuild of every part
python main.py --serve 127.0.0.1:PORT
```

The model takes about 15 s to build. Argument parsing happens before the build,
so `--help` and usage errors are instant. Keep it that way.

## Working on box/

* All dimensions are constants at the top of `main.py`, and everything else is
  derived from them. Change a constant, not derived geometry.
* Pure refactors must leave the geometry identical. Export `--step` before and
  after the change and compare each part's volume.
* `viewer.html` is a template. `serve()` fills `__PARTS__`, `__HALF__` and
  `__INFO__`. Keep those names in sync with `serve()`.
* Part names, colours and explode steps live in `PRINTED`, `REFERENCE` and
  `LOOK`. A new part needs an entry in `LOOK`.
* Format with `ruff format` (defaults) and lint with `ruff check`. The star
  import from build123d is intentional.
* Generated output (`stl/`, `box.step`) is git-ignored. Don't commit it. Put
  scratch exports outside the repo.
* Keep the module docstring in sync with the design. It's the reference for
  print orientation and assembly.

## General

* Match the surrounding style. Comments explain why, briefly.
* Don't commit or push unless asked.
