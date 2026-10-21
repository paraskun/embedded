# glim

An 8x8 RGB LED panel on the desk that shows pixel frames streamed to it over
Wi-Fi.

```
DDP over UDP ──> rx (ESP32-C3) ──I2C──> tx (RP2040) ──WS2812──> 8x8 panel
```

| part | what it is |
| --- | --- |
| `rx/` | ESP32-C3 SuperMini firmware: joins Wi-Fi, receives DDP frames, forwards them over I2C |
| `tx/` | RP2040-Zero firmware: I2C target at `0x42`, drives the panel through PIO |
| `box/` | 3D-printed desk stand for the panel and both boards, modelled in build123d |

Both boards sit on one 40 x 60 mm perfboard behind the panel, powered through a
USB-C PD trigger. The stand tilts the panel 25 deg off vertical and is
71.8 x 71.8 x 30.7 mm.

## Build

From the repository root, inside the dev shell (`nix develop`, or direnv):

```sh
cargo xtask build glim   # both firmware images
cargo xtask flash glim   # build and flash, rx first
```

The enclosure, from `box/`:

```sh
python main.py --stl     # printable parts into box/stl/
python main.py --step    # whole assembly as box/box.step
python main.py --serve   # browser viewer on http://localhost:8000/
```

The part stack, print orientation and every tunable dimension are described at
the top of `box/main.py`.
