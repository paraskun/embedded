"""Desk stand for an 8x8 WS2812 panel driven by the RP2040 + ESP32-C3 pair.

Pizza-box layout: a shallow slab with the LED face on the front and a 40 x 60
perfboard lying flat on the back plate behind it.  A separate wedge leg clips
into two dovetail grooves on the back and tilts the whole thing TILT degrees
off vertical, picture-frame style - the box rests on its bottom-back edge and
the long face of the leg lies flat on the desk.

The board stands with its long side vertical and carries the RP2040-Zero and
the ESP32-C3 SuperMini side by side on top, USB-C ports facing +X, plus the
USB-C PD trigger underneath with its port on the same wall, centred between the
other two.  That is what sets the standoff height: the trigger has to live in
the gap below the board.

Front-to-back stack, all captive, no glue:

    bezel -> diffuser -> light grid -> matrix PCB -> wiring bay -> back plate

The matrix rests on four corner posts that double as the screw bosses for the
back plate, so the LED sandwich is squeezed between those posts and the bezel.

Parts (modelled separately, shown assembled):
  * shell    - the box: bezel, walls, matrix pocket, corner posts, USB slots
  * grid     - 8x8 light divider; sits on the panel's row dividers only, the
               column gridlines and the outer rows being relieved for the
               panel's own caps and solder joints
  * diffuser - 0.8 mm sheet, print in white/natural at 100% infill
  * plate    - back cover: perfboard standoffs, vents, leg grooves
  * leg      - clip-in wedge, one per angle
  * matrix / perf / mcu_hi / mcu_lo / pd - reference solids, not printed

Print orientation: shell face-down, plate groove-side down, diffuser flat, grid
diffuser-side down so its relief channels open upward (the other way up they
would start as 56 loose islands), leg on its side with the triangular profile
on the bed.  Nothing needs support; the only bridges are the four corner-post
shelves, the USB slot ceilings and the groove roofs, all short and anchored on
two sides.

Run with --stl to write every printed part into ./stl, otherwise the module
shows itself in the OCP viewer.  All sizes are mm; Z runs from the back face
(0) toward the LEDs.
"""

import math
import sys
from pathlib import Path

from build123d import *

C, MIN = Align.CENTER, Align.MIN

# --- Matrix panel ----------------------------------------------------------
MAT = 65.0             # square WS2812 panel, 8x8 on a 8.125 pitch
MAT_PCB = 1.6
MAT_LED = 1.7          # 5050 package height above the panel
MAT_CLR = 0.4          # per side, panel -> pocket
PITCH = MAT / 8

# --- Shell -----------------------------------------------------------------
WALL = 3.0
FACE_T = 1.6           # bezel thickness
BEZEL = 1.1            # how far the bezel overlaps the panel, per side
R_CORNER = 3.0         # outer vertical edges
CH_FRONT = 1.0         # front outer edge
CH_BACK = 0.6          # back outer edge, the desk contact line
CH_WIN = 0.6           # window edge

# --- Face stack ------------------------------------------------------------
DIFF_T = 0.8
GRID_H = 5.0           # panel surface -> back of the diffuser
GRID_W = 0.9           # cell wall
GRID_RIM = 0.9
STACK_CLR = 0.2        # slack so the sandwich is never crushed
INSERT_CLR = 0.2       # per side, grid/diffuser -> pocket

# Relief where the grid would otherwise land on the panel's own parts: a
# decoupling cap sits on every column gridline, and the top and bottom rows
# carry solder joints.  The grid ends up resting on the row dividers only.
CAP = (2.0, 1.25, 1.1)         # cap envelope on a column gridline
CAP_H = 1.5                    # channel depth, cut up from the panel surface
CAP_W = 2.6                    # channel width, centred on the gridline
PAD = (3.0, 1.8)               # solder band on the top and bottom rows: depth, height
PAD_H = 2.0                    # relief depth
PAD_BAND = 4.0                 # how far in from the +/-Y edges it reaches

# --- Bay and back plate ----------------------------------------------------
BACK_T = 4.0           # back plate: thick enough for the leg grooves
PLATE_CLR = 0.2        # per side, plate -> rebate
REBATE_W = 1.4         # how far the rebate eats into the wall
POST = 5.0             # corner post, carries the panel and the plate screws
PILOT_D = 1.7          # M2 self-tap
PILOT_L = 8.0          # for the M2x8 plate screws
SCREW_D = 2.4          # plate through hole
HEAD_D = 4.4           # counterbore, keeps heads off the desk
HEAD_Z = 1.8
BOSS_R = 2.5           # Pico standoff

# --- Perfboard -------------------------------------------------------------
PERF = (40.0, 60.0)    # 4 x 6 cm, 14 x 20 points, long side along Y
PERF_T = 1.6
PERF_CLR = 0.9         # board corner -> shell corner post
PERF_IN = 3.0          # corner mount holes, centres in from each edge
PERF_HOLE = 3.2        # M3 clearance in the board itself
STANDOFF = 7.0         # plate -> board; the PD trigger lives in this gap
STANDOFF_R = 3.0
PERF_PILOT_D = 2.5     # M3 self-tap
PERF_PILOT_L = 5.5     # stays inside the standoff, clear of the leg grooves

# --- Modules ---------------------------------------------------------------
MCU = (23.5, 18.0)     # RP2040-Zero / ESP32-C3 SuperMini, long axis along X
MCU_Y = 9.5            # the two sit at +/- this; the slots follow
MCU_LIFT = 2.5         # pin header body between board and module
MCU_PCB = 1.2
MCU_USB = 3.2          # USB-C shell height above the module PCB
PD = (20.0, 15.0, 5.0) # trigger module, hangs under the board at y = 0
BAY_TOP_CLR = 2.0      # wiring room over the tallest module
MOD_OVER = 1.5         # module body hangs this far past the board's +X edge

USB_W = 13.0           # slots in the +X wall: wide enough for a plug overmold
USB_PAD = 0.8          # slop around the connector envelope
FAR = 60.0             # "well past the part", for open-ended cuts

VENT_W = 2.6           # vents run up the free strip beside the board
VENT_X = (-30.0, -25.5, -21.0, -16.5)
VENT_Y = (-10.0, 26.0)         # clear of the leg below, the screws above

# --- Leg -------------------------------------------------------------------
TILT = 25.0            # face tilt off vertical
LEG_REACH = 50.0       # how far the foot sits behind the back face
LEG_W = 46.0
LEG_R = 1.2            # profile corner fillets
RAIL_A = 11.0          # groove centres, measured up from the bottom edge
RAIL_B = 18.0
NECK = 2.4             # dovetail at the mouth
HEAD = 4.6             # dovetail at the floor
RAIL_Z = 2.8           # groove depth
RAIL_CLR = 0.15        # per side

# ---------------------------------------------------------------------------
# Derived geometry
# ---------------------------------------------------------------------------
POCKET = MAT + 2 * MAT_CLR
OUT = POCKET + 2 * WALL
HALF = OUT / 2
BOT_Y = -HALF                           # bottom-back edge, the front contact

WIN = MAT - 2 * BEZEL                   # bezel opening
GRID_XY = POCKET - 2 * INSERT_CLR
DIFF_XY = GRID_XY - 2 * INSERT_CLR

MOD_H = MCU_LIFT + MCU_PCB + MCU_USB    # perfboard surface -> top of a USB-C
BAY_Z = STANDOFF + PERF_T + MOD_H + BAY_TOP_CLR

DEPTH = BACK_T + BAY_Z + MAT_PCB + GRID_H + STACK_CLR + DIFF_T + FACE_T
Z_PCB = BACK_T + BAY_Z                  # panel underside
Z_GRID = Z_PCB + MAT_PCB                # panel surface
Z_DIFF = Z_GRID + GRID_H + STACK_CLR
Z_FACE_IN = DEPTH - FACE_T

REBATE = POCKET + 2 * REBATE_W
PLATE_XY = REBATE - 2 * PLATE_CLR

POST_OFF = POCKET / 2 - POST / 2
post_pts = [(sx * POST_OFF, sy * POST_OFF) for sx in (1, -1) for sy in (1, -1)]

Z_PERF = BACK_T + STANDOFF              # perfboard underside
Z_MOD = Z_PERF + PERF_T                 # module seating plane
# The board is as far +X as the corner posts allow, so the ports sit close to
# the wall; the modules then hang MOD_OVER past its edge to get there.
PERF_EDGE = POCKET / 2 - POST - PERF_CLR
PERF_X = PERF_EDGE - PERF[0] / 2
mount_pts = [(PERF_X + sx * (PERF[0] / 2 - PERF_IN), sy * (PERF[1] / 2 - PERF_IN))
             for sx in (1, -1) for sy in (1, -1)]

# One slot per port.  The MCU slot spans both mounting styles: modules soldered
# flat on the board, or lifted MCU_LIFT by a pin header.  The PD slot is simply
# the whole gap under the board, wherever the trigger ends up in it.
MCU_SLOT = (Z_MOD + MCU_PCB - USB_PAD, Z_MOD + MOD_H + USB_PAD)
PD_SLOT = (BACK_T + USB_PAD, Z_PERF - 0.4)
MOD_X = PERF_EDGE + MOD_OVER - MCU[0] / 2
PD_X = PERF_EDGE + MOD_OVER - PD[0] / 2

H_LEG = LEG_REACH * math.tan(math.radians(TILT))
# The box lands on the back chamfer, not on a sharp edge; lift the leg by the
# same amount so its long face and that contact line share the desk plane.
LEG_LIFT = CH_BACK * math.tan(math.radians(TILT))
LEG_Y0 = BOT_Y + LEG_LIFT
rail_ys = (BOT_Y + RAIL_A, BOT_Y + RAIL_B)
RAIL_END = LEG_W / 2 + 0.3              # groove end: stops the leg centred
assert H_LEG + LEG_LIFT > RAIL_B + HEAD / 2 + 2.0, "leg too short for groove B"


def fbox(sx, sy, z0, z1, cx=0.0, cy=0.0):
    """Axis-aligned box spanning z0..z1, centred on (cx, cy)."""
    return Pos(cx, cy, z0) * Box(sx, sy, z1 - z0, align=(C, C, MIN))


def yz(*pts):
    """Sketch from (y, z) points, placed in the YZ plane, normal along +X."""
    return Plane.YZ * Polygon(*pts, align=None)


def rail(cy, neck, head, depth, x0, x1, lead=0.0):
    """Dovetail bar along X: narrow (neck) at z=0, wide (head) at z=depth."""
    pts = [(cy - neck / 2, -lead), (cy + neck / 2, -lead)]
    if lead:
        pts.append((cy + neck / 2, 0.0))
    pts += [(cy + head / 2, depth), (cy - head / 2, depth)]
    if lead:
        pts.append((cy - neck / 2, 0.0))
    return Pos(x0, 0, 0) * extrude(yz(*pts), amount=x1 - x0, dir=(1, 0, 0))


# ---------------------------------------------------------------------------
# Shell: bezel, walls, matrix pocket, corner posts
# ---------------------------------------------------------------------------
shell = Box(OUT, OUT, DEPTH, align=(C, C, MIN))
shell = fillet(shell.edges().filter_by(Axis.Z), R_CORNER)
shell = chamfer(shell.faces().sort_by(Axis.Z)[-1].edges(), CH_FRONT)
shell = chamfer(shell.faces().sort_by(Axis.Z)[0].edges(), CH_BACK)

shell -= fbox(POCKET, POCKET, -1, Z_FACE_IN)        # pocket and wiring bay
shell -= fbox(WIN, WIN, Z_FACE_IN, DEPTH + 1)       # bezel window
shell -= fbox(REBATE, REBATE, -1, BACK_T)           # back plate rebate

for px, py in post_pts:                             # panel rest / screw boss
    shell += fbox(POST, POST, BACK_T, Z_PCB, cx=px, cy=py)
    shell -= Pos(px, py, BACK_T) * Cylinder(PILOT_D / 2, PILOT_L, align=(C, C, MIN))

wall_x = POCKET / 2 + WALL / 2                      # USB access, +X wall
for cy in (MCU_Y, -MCU_Y):                          # the two modules, on top
    shell -= fbox(WALL + 2, USB_W, *MCU_SLOT, cx=wall_x, cy=cy)
shell -= fbox(WALL + 2, USB_W, *PD_SLOT, cx=wall_x) # PD trigger, underneath

front = shell.edges().group_by(Axis.Z)[-1]
shell = chamfer(ShapeList(e for e in front if abs(e.length - WIN) < 0.5), CH_WIN)

# ---------------------------------------------------------------------------
# Light grid: 8x8 cells on the LED pitch, clipped to leave an even rim
# ---------------------------------------------------------------------------
grid = fbox(GRID_XY, GRID_XY, Z_GRID, Z_GRID + GRID_H)
cells = None
for i in range(8):
    for j in range(8):
        cell = fbox(PITCH - GRID_W, PITCH - GRID_W, Z_GRID - 1, Z_GRID + GRID_H + 1,
                    cx=(i - 3.5) * PITCH, cy=(j - 3.5) * PITCH)
        cells = cell if cells is None else cells + cell
inner = GRID_XY - 2 * GRID_RIM
grid -= cells & fbox(inner, inner, Z_GRID - 2, Z_GRID + GRID_H + 2)

for i in range(9):                                  # clear the caps: every
    grid -= fbox(CAP_W, GRID_XY + 2, Z_GRID, Z_GRID + CAP_H,   # column gridline,
                 cx=(i - 4) * PITCH)                # rim included
for sy in (1, -1):                                  # clear the row solder joints
    grid -= fbox(GRID_XY + 2, PAD_BAND, Z_GRID, Z_GRID + PAD_H,
                 cy=sy * (GRID_XY / 2 - PAD_BAND / 2))

# ---------------------------------------------------------------------------
# Diffuser
# ---------------------------------------------------------------------------
diffuser = fbox(DIFF_XY, DIFF_XY, Z_DIFF, Z_DIFF + DIFF_T)

# ---------------------------------------------------------------------------
# Back plate: board mounts, vents, leg grooves
# ---------------------------------------------------------------------------
plate = fbox(PLATE_XY, PLATE_XY, 0, BACK_T)
plate = fillet(plate.edges().filter_by(Axis.Z), R_CORNER - 1.0)

for px, py in post_pts:
    plate -= Pos(px, py, -1) * Cylinder(SCREW_D / 2, BACK_T + 2, align=(C, C, MIN))
    plate -= Pos(px, py, -1) * Cylinder(HEAD_D / 2, HEAD_Z + 1, align=(C, C, MIN))

for mx, my in mount_pts:                            # perfboard standoffs
    plate += Pos(mx, my, BACK_T) * Cylinder(STANDOFF_R, STANDOFF, align=(C, C, MIN))
    plate -= Pos(mx, my, Z_PERF - PERF_PILOT_L) * Cylinder(
        PERF_PILOT_D / 2, PERF_PILOT_L, align=(C, C, MIN))

for vx in VENT_X:                                   # vents beside the board
    plate -= fbox(VENT_W, VENT_Y[1] - VENT_Y[0], -1, BACK_T + 1,
                  cx=vx, cy=sum(VENT_Y) / 2)

for cy in rail_ys:                                  # leg grooves, slide in from -X
    plate -= rail(cy, NECK, HEAD, RAIL_Z, -PLATE_XY / 2 - 1, RAIL_END, lead=0.8)

# ---------------------------------------------------------------------------
# Leg: right triangle, hypotenuse on the desk, dovetails into the plate
# ---------------------------------------------------------------------------
profile = Polygon((LEG_Y0, 0), (LEG_Y0 + H_LEG, 0), (LEG_Y0 + H_LEG, -LEG_REACH),
                  align=None)
profile = fillet(profile.vertices(), LEG_R)
leg = Pos(-LEG_W / 2, 0, 0) * extrude(Plane.YZ * profile, amount=LEG_W, dir=(1, 0, 0))

for cy in rail_ys:                                  # tongues
    leg += rail(cy, NECK - 2 * RAIL_CLR, HEAD - 2 * RAIL_CLR, RAIL_Z - 0.2,
                -LEG_W / 2, LEG_W / 2)

# ---------------------------------------------------------------------------
# Reference solids (not printed)
# ---------------------------------------------------------------------------
matrix = fbox(MAT, MAT, Z_PCB, Z_PCB + MAT_PCB)
for i in range(8):
    for j in range(8):
        matrix += fbox(5.0, 5.0, Z_GRID, Z_GRID + MAT_LED,
                       cx=(i - 3.5) * PITCH, cy=(j - 3.5) * PITCH)
for i in range(1, 8):                               # caps on the column gridlines
    for j in range(8):
        matrix += fbox(CAP[0], CAP[1], Z_GRID, Z_GRID + CAP[2],
                       cx=(i - 4) * PITCH, cy=(j - 3.5) * PITCH)
for sy in (1, -1):                                  # solder along the outer rows,
    matrix += fbox(MAT, PAD[0], Z_GRID, Z_GRID + PAD[1],        # worst case: a
                   cy=sy * (MAT / 2 - PAD[0] / 2))              # full-width band


perf = fbox(*PERF, Z_PERF, Z_MOD, cx=PERF_X)
for mx, my in mount_pts:
    perf -= Pos(mx, my, Z_PERF - 1) * Cylinder(PERF_HOLE / 2, PERF_T + 2,
                                               align=(C, C, MIN))


def board(size, thick, cx, cy, z0, usb=(9.0, 7.3, 3.2)):
    """Module footprint plus the USB-C shell poking out of its +X end."""
    b = fbox(size[0], size[1], z0, z0 + thick, cx=cx, cy=cy)
    b += fbox(usb[0], usb[1], z0 + thick, z0 + thick + usb[2],
              cx=cx + size[0] / 2 - usb[0] / 2 + 1.0, cy=cy)
    return b


mcu_hi = board(MCU, MCU_PCB, MOD_X, MCU_Y, Z_MOD + MCU_LIFT)
mcu_lo = board(MCU, MCU_PCB, MOD_X, -MCU_Y, Z_MOD + MCU_LIFT)
# Trigger under the board: envelope only, port centred in the gap.
pd = fbox(PD[0], PD[1], Z_PERF - PD[2], Z_PERF, cx=PD_X)
pd += fbox(9.0, 7.3, Z_PERF - PD[2] / 2 - 1.6, Z_PERF - PD[2] / 2 + 1.6,
           cx=PD_X + PD[0] / 2 - 9.0 / 2 + 1.0)

PRINTED = {"shell": shell, "plate": plate, "grid": grid,
           "diffuser": diffuser, "leg": leg}

if __name__ == "__main__":
    if "--stl" in sys.argv:
        out = Path(__file__).parent / "stl"
        out.mkdir(exist_ok=True)
        print(f"box {OUT:.1f} x {OUT:.1f} x {DEPTH:.1f}, "
              f"tilted {TILT:.0f} deg off vertical")
        for name, part in PRINTED.items():
            export_stl(part, str(out / f"{name}.stl"))
            b = part.bounding_box()
            print(f"{name:9s} {b.size.X:5.1f} x {b.size.Y:5.1f} x {b.size.Z:5.1f}"
                  f"  {part.volume / 1000:6.1f} cm3  -> {out.name}/{name}.stl")
    else:
        try:
            from ocp_vscode import show_all
            show_all()
        except Exception as exc:                     # headless: nothing to show
            print(f"viewer unavailable ({exc}); pass --stl to export", file=sys.stderr)
