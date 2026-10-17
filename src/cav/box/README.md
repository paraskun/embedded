# box

Desk enclosure for `cav`: an 8x8 WS2812 panel on the front, a 40 x 60 perfboard
carrying the RP2040 (`tx`), the ESP32-C3 (`rx`) and a USB-C PD trigger behind
it, and a clip-in wedge leg that stands the whole thing up at 25 deg off
vertical.

Outer size 71.8 x 71.8 x 30.7 mm; on the desk it occupies 72 x 79 mm and stands
78 mm tall.

## Build

```sh
python main.py --stl     # writes stl/{shell,plate,grid,diffuser,leg}.stl
python main.py           # shows the assembly in the OCP viewer
```

Needs `build123d` (and `ocp_vscode` for the viewer) from `requirements.txt`.

## Parts

| part | notes |
| --- | --- |
| `shell` | bezel, walls, matrix pocket, four corner posts, two USB slots |
| `plate` | back cover: four perfboard standoffs, vents, leg grooves |
| `grid` | 8x8 light divider on the LED pitch, 5 mm deep, relieved for the panel's caps and solder |
| `diffuser` | 0.8 mm sheet; print white or natural at 100% infill |
| `leg` | the wedge; print one per angle you want |

Front to back the stack is bezel, diffuser, grid, panel, wiring bay, plate.
Nothing is glued: the panel rests on the corner posts and the sandwich is
trapped between those posts and the bezel once the plate is screwed on.

Shell face-down, plate groove-side down, diffuser flat, leg on its side with
the triangular profile on the bed. No supports: the only bridges are the
corner-post shelves, the USB slot ceilings and the groove roofs.

Print the grid **diffuser-side down**, so its relief channels open upward. The
other way up, the first 1.5 mm would be 56 unconnected islands.

0.2 mm layers, 3 perimeters. Print the grid in black (or anything opaque) to
keep the pixels crisp.

## Panel relief

The panel carries a decoupling cap on every column gridline and solder joints
along the top and bottom rows, so the grid cannot sit flat on it. It is cut
back to clear both and lands on the row dividers only - 56 pads, 5 mm2 each,
278 mm2 of contact in total, which is plenty to keep it square:

* `CAP_H` 1.5 mm, `CAP_W` 2.6 mm - a channel along every column gridline,
  running the full height and through the left and right rim, in case the outer
  columns' caps sit under it.
* `PAD_H` 2.0 mm, `PAD_BAND` 4.0 mm - a band along the top and bottom edges.

Both are cut from the panel face upward, so the dividers still stand 3.5 mm
and 3.0 mm proud there and keep blocking crosstalk. The reference panel models
the caps (2.0 x 1.25 x 1.1) and a full-width 1.8 mm solder band on the outer
rows, so `grid & matrix` is a real check - raise `CAP_H` or `PAD_H` if your
panel's parts are taller.

## Hardware

* 8x8 WS2812 panel, 65 x 65 mm
* 40 x 60 mm perfboard, 14 x 20 points, mount hole in each corner, standing
  with its long side vertical
* RP2040-Zero and ESP32-C3 SuperMini on top, USB-C ports facing +X
* USB-C PD trigger under the board, port on the same wall between the other two
* 4x M3x8 self-tapping screws for the perfboard
* 4x M2x8 self-tapping screws for the back plate

The board stands on 7 mm standoffs so the trigger fits underneath; that gap,
plus the module height above the board, is what sets the box thickness. It sits
as far toward the +X wall as the shell's corner posts allow (0.9 mm clear), and
the modules hang 1.5 mm past its edge, which leaves their connectors 3.4 mm
behind the wall's inner face. The three USB slots are oversized to suit: a
plug's overmold enters the slot, so the recess does not stop it seating.

## Assemble

1. Build the perfboard: modules on top with their USB-C ports at the +X edge
   and 19 mm apart in Y, trigger underneath with its port centred between them.
   The modules overhang the board's +X edge so the ports clear it.
2. Screw the board onto the four standoffs with M3x8.
3. Wire the boards to each other and to the panel; the panel's pads face into
   the bay, and the wiring runs around the board, not over the modules.
4. Drop the diffuser into the shell, then the grid, then the panel face-down
   onto the corner posts.
5. Fit the plate into the rebate and drive the four M2x8 screws into the posts.
6. Slide the leg onto the back from the left until it bottoms out; it ends up
   centred.

## Tuning

Everything is derived from the constants at the top of `main.py`:

* `TILT` / `LEG_REACH` - stand angle and how far the foot sits back. The leg
  height follows from both, so a second leg at a different `TILT` drops into
  the same grooves.
* `MAT`, `MAT_PCB` - panel size, if yours is not 65 mm square.
* `PERF`, `PERF_IN`, `PERF_PILOT_D` - board size, corner hole inset and screw;
  the standoffs follow. Default is M3 holes 3 mm in from each edge.
* `STANDOFF` - 7 mm, set by the PD trigger under the board.
* `MCU_Y` - half the spacing between the two module USB ports; the two upper
  wall slots are centred on it.
* `MCU_LIFT` - 2.5 mm for modules on pin headers, 0 if soldered flat; drop it
  and the box gets thinner by the same amount.
* `INSERT_CLR`, `PLATE_CLR`, `RAIL_CLR` - fit clearances, per side.
* `STACK_CLR` - slack in the LED sandwich; raise it if the diffuser bulges.
