"""Desk stand for an 8x8 WS2812 panel driven by the RP2040 + ESP32-C3 pair.

Pizza-box layout: a shallow slab with the LED face on the front and a 40 x 60
perfboard lying flat on the back plate behind it.  A separate wedge leg clips
into two dovetail grooves on the back and tilts the whole thing TILT degrees
off vertical, picture-frame style - the box rests on its bottom-back edge and
the long face of the leg lies flat on the desk.

The board stands with its long side vertical and carries the RP2040-Zero and
the ESP32-C3 SuperMini side by side on top on tall headers, USB-C ports facing
+X, plus the USB-C PD trigger hanging upside down underneath with its port on
the same wall, centred between the other two.  The trigger's port sets the
standoff height and the MCU ports the bay depth; both are measured off the
soldered-up board.

Front-to-back stack, all captive, no glue:

    bezel -> diffuser -> light grid -> matrix PCB -> wiring bay -> back plate

The matrix rests on four corner pillars that stand up off the back plate, so
screwing the plate on squeezes the LED sandwich between them and the bezel.
The plate is full size and sits flush on the shell's back edge; its screws
go into four bosses on the shell walls, clear of the board.

Parts (modelled separately, shown assembled):
  * shell    - the box: bezel, walls, matrix pocket, screw bosses, PD USB slot
  * grid     - 8x8 light divider; sits on the panel's row dividers only, the
               column gridlines and the first and last row gridlines being
               relieved for the panel's own caps
  * diffuser - 0.8 mm sheet, print in white/natural at 100% infill
  * plate    - back cover: panel pillars, perfboard standoffs, vents,
               leg grooves
  * leg      - clip-in wedge, one per angle
  * matrix / perf / mcu_hi / mcu_lo / pd - reference solids, not printed

Print orientation: shell open side down, so the screw bosses grow straight
up off the bed; plate groove-side down, diffuser flat, grid diffuser-side down
so its relief channels open upward (the other way up they would start as 56
loose islands), leg on its side with the triangular profile on the bed.
Nothing needs support: the bezel's inner lip is a short overhang off the
walls, and the only bridges are the USB slot ceiling and the groove roofs,
all short and anchored on two sides.

Run with --help for the export and viewer options; with no options the module
shows itself in the OCP viewer.  All sizes are mm; Z runs from the back face
(0) toward the LEDs.
"""

import argparse
import json
import math
import sys
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

HERE = Path(__file__).parent


# ---------------------------------------------------------------------------
# Command line
# ---------------------------------------------------------------------------
def address(text):
    """argparse type for --serve: "[HOST:]PORT" -> (host, port)."""
    host, _, port = text.rpartition(":")
    try:
        port = int(port or 8000)
    except ValueError:
        raise argparse.ArgumentTypeError(f"bad port in {text!r}") from None
    return host or "localhost", port


def cli():
    """Argument parser; at most one action, none means the OCP viewer."""
    parser = argparse.ArgumentParser(
        description=__doc__.split("\n\n")[0].replace("\n", " "),
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
examples:
  python main.py                      show the assembly in the OCP viewer
  python main.py --stl                write stl/{shell,plate,grid,diffuser,leg}.stl
  python main.py --step               write box.step next to this script
  python main.py --step glim.step     write the STEP assembly to glim.step
  python main.py --serve 0.0.0.0:8080 browser viewer, reachable from the LAN""",
    )
    action = parser.add_mutually_exclusive_group()
    action.add_argument(
        "--stl",
        action="store_true",
        help=f"write every printed part to {HERE.name}/stl/<part>.stl",
    )
    action.add_argument(
        "--step",
        nargs="?",
        const=HERE / "box.step",
        type=Path,
        metavar="PATH",
        help="write all parts, printed and reference, as one STEP assembly "
        "(default: box.step next to this script)",
    )
    action.add_argument(
        "--serve",
        nargs="?",
        const=("localhost", 8000),
        type=address,
        metavar="[HOST:]PORT",
        help="serve a three.js viewer of the assembly "
        "(default: localhost:8000; HOST defaults to localhost)",
    )
    return parser


# Parse before building: the model takes several seconds to build, and --help
# or a typo should not have to wait for it.
ARGS = cli().parse_args() if __name__ == "__main__" else None

from build123d import *  # noqa: E402, F403 - build123d's documented idiom

C, MIN = Align.CENTER, Align.MIN

# --- Matrix panel ----------------------------------------------------------
MAT = 65.0  # square WS2812 panel, 8x8
MAT_PCB = 1.6
MAT_LED = 1.7  # 5050 package height above the panel
LED = 5.0  # 5050 package footprint
LED_GAP = 3.0  # between neighbouring packages
MAT_CLR = 0.4  # per side, panel -> pocket
PITCH = LED + LED_GAP

# --- Shell -----------------------------------------------------------------
WALL = 3.0
FACE_T = 1.6  # bezel thickness
BEZEL = 1.1  # how far the bezel overlaps the panel, per side
R_CORNER = 3.0  # outer vertical edges
CH_FRONT = 1.0  # front outer edge
CH_BACK = 0.6  # plate's back outer edge, the desk contact line
CH_WIN = 0.6  # window edge

# --- Face stack ------------------------------------------------------------
DIFF_T = 0.8
GRID_H = 5.0  # panel surface -> back of the diffuser
GRID_W = 0.9  # cell wall
GRID_RIM = 0.9
STACK_CLR = 0.2  # slack so the sandwich is never crushed
INSERT_CLR = 0.2  # per side, grid/diffuser -> pocket

# Relief where the grid would otherwise land on the panel's own parts: a
# decoupling cap sits on every column gridline next to each LED, and on the
# first and last row gridlines next to each LED as well.  The grid ends up
# resting on the remaining row dividers only.
CAP = (2.0, 1.25, 1.1)  # cap envelope on a column gridline
CAP_ROWS = (1, 7)  # row gridlines that carry caps too
CAP_H = 1.5  # channel depth, cut up from the panel surface
CAP_W = 2.6  # channel width, centred on the gridline

# --- Bay and back plate ----------------------------------------------------
BACK_T = 4.0  # back plate: thick enough for the leg grooves
POST = 5.0  # corner pillar on the plate, the panel rests on it
POST_CLR = 0.4  # per side, pillar -> pocket wall
BOSS = 5.0  # square screw boss on the shell wall, tapered off at 45 deg
BOSS_AT = 24.0  # boss centre along its wall, from the wall's middle
PILOT_D = 1.7  # M2 self-tap
PILOT_L = 8.0  # for the M2x8 plate screws
SCREW_D = 2.4  # plate through hole
HEAD_D = 4.4  # counterbore, keeps heads off the desk
HEAD_Z = 1.8
BOSS_R = 2.5  # Pico standoff

# --- Perfboard -------------------------------------------------------------
PERF = (40.0, 60.0)  # 4 x 6 cm, 14 x 20 points, long side along Y
PERF_T = 1.5
PERF_CLR = 0.9  # board corner -> shell corner post
PERF_IN = 3.0  # corner mount holes, centres in from each edge
PERF_HOLE = 3.2  # M3 clearance in the board itself
PD_CLR = 1.0  # PD port -> back plate; sets the standoff height
STANDOFF_R = 3.0
PERF_PILOT_D = 2.5  # M3 self-tap
PERF_PILOT_L = 5.5  # stays inside the standoff, clear of the leg grooves

# --- Modules ---------------------------------------------------------------
# Port positions as measured on the soldered-up board, looking at its +X edge
# with the MCUs on top: left is -Y, right is +Y, heights are off the board's
# own faces.
USB_C = (7.3, 8.9, 3.2)  # receptacle shell: length, width, height
MCU_PORT_L = 15.0  # board's left end -> left MCU port's left side
MCU_PORT_R = 14.5  # board's right end -> right MCU port's right side
MCU_PORT_Z = 12.0  # board top -> MCU port underside; tall headers
PD_PORT_L = 25.5  # board's left end -> PD port's left side
PD_PORT_Z = 7.0  # board underside -> PD port underside, hanging

MCU = (23.5, 18.0)  # RP2040-Zero / ESP32-C3 SuperMini, long axis along X
MCU_PCB = 1.2
PD = (20.0, 15.0)  # trigger module, port on the far side of its PCB
PD_PCB = 1.2
BAY_TOP_CLR = 2.0  # wiring room over the tallest module
MOD_OVER = 1.5  # module body hangs this far past the board's +X edge

USB_W = 13.0  # PD slot in the +X wall: room for a plug overmold
USB_H = 7.0
FAR = 60.0  # "well past the part", for open-ended cuts

VENT_W = 2.6  # vents run up the free strip beside the board
VENT_X = (-30.0, -25.5, -21.0, -16.5)
VENT_Y = (-10.0, 26.0)  # clear of the leg below, the screws above

# --- Leg -------------------------------------------------------------------
TILT = 25.0  # face tilt off vertical
LEG_REACH = 50.0  # how far the foot sits behind the back face
LEG_W = 46.0
LEG_R = 1.2  # profile corner fillets
RAIL_A = 11.0  # groove centres, measured up from the bottom edge
RAIL_B = 18.0
NECK = 2.4  # dovetail at the mouth
HEAD = 4.6  # dovetail at the floor
RAIL_Z = 2.8  # groove depth
RAIL_CLR = 0.15  # per side

# ---------------------------------------------------------------------------
# Derived geometry
# ---------------------------------------------------------------------------
POCKET = MAT + 2 * MAT_CLR
OUT = POCKET + 2 * WALL
HALF = OUT / 2
BOT_Y = -HALF  # bottom-back edge, the front contact

WIN = MAT - 2 * BEZEL  # bezel opening
GRID_XY = POCKET - 2 * INSERT_CLR
DIFF_XY = GRID_XY - 2 * INSERT_CLR

STANDOFF = PD_PORT_Z + PD_CLR  # plate -> board; the PD trigger hangs in here
MOD_H = MCU_PORT_Z + USB_C[2]  # perfboard surface -> top of an MCU port
BAY_Z = STANDOFF + PERF_T + MOD_H + BAY_TOP_CLR

DEPTH = BACK_T + BAY_Z + MAT_PCB + GRID_H + STACK_CLR + DIFF_T + FACE_T
Z_PCB = BACK_T + BAY_Z  # panel underside
Z_GRID = Z_PCB + MAT_PCB  # panel surface
Z_DIFF = Z_GRID + GRID_H + STACK_CLR
Z_FACE_IN = DEPTH - FACE_T


POST_OFF = POCKET / 2 - POST_CLR - POST / 2
post_pts = [(sx * POST_OFF, sy * POST_OFF) for sx in (1, -1) for sy in (1, -1)]

# Screw bosses, as (wall angle about Z, offset along the wall): two on the +X
# wall beside the ports, two on the +/-Y walls at the -X end.  The board fills
# the +X end of the Y walls, and the leg grooves cross the lower -X corner.
boss_at = [(0, BOSS_AT), (0, -BOSS_AT), (90, BOSS_AT), (-90, -BOSS_AT)]
BOSS_OFF = POCKET / 2 - BOSS / 2
Z_BOSS = BACK_T + PILOT_L + 1.0  # full section up to here, then the taper
screw_pts = [
    (
        BOSS_OFF * math.cos(math.radians(a)) - s * math.sin(math.radians(a)),
        BOSS_OFF * math.sin(math.radians(a)) + s * math.cos(math.radians(a)),
    )
    for a, s in boss_at
]

Z_PERF = BACK_T + STANDOFF  # perfboard underside
Z_MOD = Z_PERF + PERF_T  # module seating plane
# The board is as far +X as the corner posts allow, so the ports sit close to
# the wall; the modules then hang MOD_OVER past its edge to get there.
PERF_EDGE = POCKET / 2 - POST_CLR - POST - PERF_CLR
PERF_X = PERF_EDGE - PERF[0] / 2
mount_pts = [
    (PERF_X + sx * (PERF[0] / 2 - PERF_IN), sy * (PERF[1] / 2 - PERF_IN))
    for sx in (1, -1)
    for sy in (1, -1)
]

# Port centres along Y, from the measured edge offsets; each module is centred
# on its port.
MCU_Y_LO = -PERF[1] / 2 + MCU_PORT_L + USB_C[1] / 2
MCU_Y_HI = PERF[1] / 2 - MCU_PORT_R - USB_C[1] / 2
PD_Y = -PERF[1] / 2 + PD_PORT_L + USB_C[1] / 2
MOD_X = PERF_EDGE + MOD_OVER - MCU[0] / 2
PD_X = PERF_EDGE + MOD_OVER - PD[0] / 2

# Only the PD port gets a slot, centred on it; the MCU ports stay closed in.
# The slot would dip below the shell, so it stops there and the plate closes
# it off.
Z_PD_PORT = Z_PERF - PD_PORT_Z + USB_C[2] / 2
PD_SLOT = (max(BACK_T, Z_PD_PORT - USB_H / 2), Z_PD_PORT + USB_H / 2)

H_LEG = LEG_REACH * math.tan(math.radians(TILT))
# The box lands on the back chamfer, not on a sharp edge; lift the leg by the
# same amount so its long face and that contact line share the desk plane.
LEG_LIFT = CH_BACK * math.tan(math.radians(TILT))
LEG_Y0 = BOT_Y + LEG_LIFT
rail_ys = (BOT_Y + RAIL_A, BOT_Y + RAIL_B)
RAIL_END = LEG_W / 2 + 0.3  # groove end: stops the leg centred
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
# Shell: bezel, walls, matrix pocket, screw bosses
# ---------------------------------------------------------------------------
# Solid block first, rounded and chamfered while its edges are still simple,
# then hollowed out from the back.  It starts where the plate ends: the two
# meet flush, so the shell prints open side down with every boss on the bed.
shell = fbox(OUT, OUT, BACK_T, DEPTH)
shell = fillet(shell.edges().filter_by(Axis.Z), R_CORNER)
shell = chamfer(shell.edges().group_by(Axis.Z)[-1], CH_FRONT)

shell -= fbox(POCKET, POCKET, BACK_T - 1, Z_FACE_IN)  # pocket and wiring bay
shell -= fbox(WIN, WIN, Z_FACE_IN, DEPTH + 1)  # bezel window

# Plate screw bosses, standing on the shell's back edge.  The front end slopes
# back into the wall at 45 deg so the face-down orientation works too.  Drawn
# in XZ for the +X wall, overlapping into it, then turned onto its own wall.
x0, xw = POCKET / 2 - BOSS, POCKET / 2 + 0.5
taper = Plane.XZ * Polygon(
    (x0, BACK_T),
    (xw, BACK_T),
    (xw, Z_BOSS + xw - x0),
    (x0, Z_BOSS),
    align=None,
)
taper = extrude(taper, amount=BOSS / 2, both=True)
for (a, s), (bx, by) in zip(boss_at, screw_pts):
    shell += Rot(0, 0, a) * Pos(0, s, 0) * taper
    shell -= Pos(bx, by, BACK_T) * Cylinder(PILOT_D / 2, PILOT_L, align=(C, C, MIN))

wall_x = POCKET / 2 + WALL / 2  # power input, +X wall
shell -= fbox(WALL + 2, USB_W, *PD_SLOT, cx=wall_x, cy=PD_Y)

# Break the window edge: of the front-face edges, keep only the four that
# bound the bezel opening (the outer ones are already chamfered).
front = shell.edges().group_by(Axis.Z)[-1]
shell = chamfer(ShapeList(e for e in front if abs(e.length - WIN) < 0.5), CH_WIN)

# ---------------------------------------------------------------------------
# Light grid: 8x8 cells on the LED pitch, clipped to leave an even rim
# ---------------------------------------------------------------------------
# One slab, minus everything inside the rim that is not a gridline wall, so
# the outer cells run out to the rim whatever the pitch.
grid = fbox(GRID_XY, GRID_XY, Z_GRID, Z_GRID + GRID_H)
inner = GRID_XY - 2 * GRID_RIM
cells = fbox(inner, inner, Z_GRID - 1, Z_GRID + GRID_H + 1)
for k in range(1, 8):  # inner gridlines, between LED rows and columns
    cells -= fbox(GRID_W, GRID_XY, Z_GRID - 2, Z_GRID + GRID_H + 2, cx=(k - 4) * PITCH)
    cells -= fbox(GRID_XY, GRID_W, Z_GRID - 2, Z_GRID + GRID_H + 2, cy=(k - 4) * PITCH)
grid -= cells

# Clear the caps: a channel up from the panel face along every column
# gridline, rim included, and along the capped row gridlines.
for i in range(9):
    grid -= fbox(CAP_W, GRID_XY + 2, Z_GRID, Z_GRID + CAP_H, cx=(i - 4) * PITCH)
for j in CAP_ROWS:
    grid -= fbox(GRID_XY + 2, CAP_W, Z_GRID, Z_GRID + CAP_H, cy=(j - 4) * PITCH)

# ---------------------------------------------------------------------------
# Diffuser
# ---------------------------------------------------------------------------
diffuser = fbox(DIFF_XY, DIFF_XY, Z_DIFF, Z_DIFF + DIFF_T)

# ---------------------------------------------------------------------------
# Back plate: board mounts, vents, leg grooves
# ---------------------------------------------------------------------------
plate = fbox(OUT, OUT, 0, BACK_T)  # flush with the shell all round
plate = fillet(plate.edges().filter_by(Axis.Z), R_CORNER)
plate = chamfer(plate.edges().group_by(Axis.Z)[0], CH_BACK)

for px, py in post_pts:  # panel rests: the plate prints back-down, so they
    plate += fbox(POST, POST, BACK_T, Z_PCB, cx=px, cy=py)  # grow straight up

for bx, by in screw_pts:  # screw clearance, counterbored from the desk side
    plate -= Pos(bx, by, -1) * Cylinder(SCREW_D / 2, BACK_T + 2, align=(C, C, MIN))
    plate -= Pos(bx, by, -1) * Cylinder(HEAD_D / 2, HEAD_Z + 1, align=(C, C, MIN))

for mx, my in mount_pts:  # perfboard standoffs, pilot drilled from the top
    plate += Pos(mx, my, BACK_T) * Cylinder(STANDOFF_R, STANDOFF, align=(C, C, MIN))
    plate -= Pos(mx, my, Z_PERF - PERF_PILOT_L) * Cylinder(
        PERF_PILOT_D / 2, PERF_PILOT_L, align=(C, C, MIN)
    )

vent_l, vent_y = VENT_Y[1] - VENT_Y[0], sum(VENT_Y) / 2
for vx in VENT_X:  # vents beside the board
    plate -= fbox(VENT_W, vent_l, -1, BACK_T + 1, cx=vx, cy=vent_y)

for cy in rail_ys:  # leg grooves, slide in from -X
    plate -= rail(cy, NECK, HEAD, RAIL_Z, -OUT / 2 - 1, RAIL_END, lead=0.8)

# ---------------------------------------------------------------------------
# Leg: right triangle, hypotenuse on the desk, dovetails into the plate
# ---------------------------------------------------------------------------
# The profile is drawn in (y, z): the right angle sits on the back face at the
# bottom, one leg runs up the back, the other runs LEG_REACH out behind it.
profile = Polygon(
    (LEG_Y0, 0), (LEG_Y0 + H_LEG, 0), (LEG_Y0 + H_LEG, -LEG_REACH), align=None
)
profile = fillet(profile.vertices(), LEG_R)
leg = Pos(-LEG_W / 2, 0, 0) * extrude(Plane.YZ * profile, amount=LEG_W, dir=(1, 0, 0))

# Tongues: the grooves less the fit clearance, 0.2 short so they never bottom.
neck, head = NECK - 2 * RAIL_CLR, HEAD - 2 * RAIL_CLR
for cy in rail_ys:
    leg += rail(cy, neck, head, RAIL_Z - 0.2, -LEG_W / 2, LEG_W / 2)

# ---------------------------------------------------------------------------
# Reference solids (not printed)
# ---------------------------------------------------------------------------
# The panel: PCB, LEDs and the caps the grid has to clear, so `grid & matrix`
# is a real interference check.
matrix = fbox(MAT, MAT, Z_PCB, Z_PCB + MAT_PCB)
Z_LED, Z_CAP = Z_GRID + MAT_LED, Z_GRID + CAP[2]
for i in range(8):
    for j in range(8):
        cx, cy = (i - 3.5) * PITCH, (j - 3.5) * PITCH
        matrix += fbox(LED, LED, Z_GRID, Z_LED, cx=cx, cy=cy)
for i in range(1, 8):  # caps on the column gridlines
    for j in range(8):
        cx, cy = (i - 4) * PITCH, (j - 3.5) * PITCH
        matrix += fbox(CAP[0], CAP[1], Z_GRID, Z_CAP, cx=cx, cy=cy)
for j in CAP_ROWS:  # and on two row gridlines, turned to match
    for i in range(8):
        cx, cy = (i - 3.5) * PITCH, (j - 4) * PITCH
        matrix += fbox(CAP[1], CAP[0], Z_GRID, Z_CAP, cx=cx, cy=cy)

# Perfboard with its four mount holes.

perf = fbox(*PERF, Z_PERF, Z_MOD, cx=PERF_X)
for mx, my in mount_pts:
    perf -= Pos(mx, my, Z_PERF - 1) * Cylinder(
        PERF_HOLE / 2, PERF_T + 2, align=(C, C, MIN)
    )


def board(size, thick, cx, cy, z0, usb=USB_C):
    """Module footprint plus the USB-C shell poking out of its +X end."""
    b = fbox(size[0], size[1], z0, z0 + thick, cx=cx, cy=cy)
    usb_x = cx + size[0] / 2 - usb[0] / 2 + 1.0  # overhangs the PCB by 1 mm
    b += fbox(usb[0], usb[1], z0 + thick, z0 + thick + usb[2], cx=usb_x, cy=cy)
    return b


Z_MCU = Z_MOD + MCU_PORT_Z - MCU_PCB  # module PCB underside
mcu_hi = board(MCU, MCU_PCB, MOD_X, MCU_Y_HI, Z_MCU)
mcu_lo = board(MCU, MCU_PCB, MOD_X, MCU_Y_LO, Z_MCU)
# Trigger under the board, upside down: its port faces the plate.
z_pd = Z_PERF - PD_PORT_Z + USB_C[2]  # PD PCB underside, port side
pd = fbox(*PD, z_pd, z_pd + PD_PCB, cx=PD_X, cy=PD_Y)
pd_port_x = PD_X + PD[0] / 2 - USB_C[0] / 2 + 1.0
pd += fbox(*USB_C[:2], z_pd - USB_C[2], z_pd, cx=pd_port_x, cy=PD_Y)

# ---------------------------------------------------------------------------
# Part tables: what gets exported, and how the browser viewer draws it
# ---------------------------------------------------------------------------
PRINTED = {  # written by --stl; all parts go to --step and --serve
    "shell": shell,
    "plate": plate,
    "grid": grid,
    "diffuser": diffuser,
    "leg": leg,
}
REFERENCE = {
    "matrix": matrix,
    "perf": perf,
    "mcu_hi": mcu_hi,
    "mcu_lo": mcu_lo,
    "pd": pd,
}

# Browser viewer: colour, opacity and explode step (along Z) per part.
LOOK = {
    "shell": ("#e07a3f", 1.0, 4),
    "plate": ("#c9662f", 1.0, -1),
    "grid": ("#303236", 1.0, 2),
    "diffuser": ("#f4f4f0", 0.75, 3),
    "leg": ("#d9a441", 1.0, -2),
    "matrix": ("#1f6f3f", 0.8, 1),
    "perf": ("#b5894a", 0.8, 0),
    "mcu_hi": ("#2552a3", 0.8, 0),
    "mcu_lo": ("#2552a3", 0.8, 0),
    "pd": ("#7a2fa3", 0.8, 0),
}


def write_stl(out):
    """Write each printed part to out/<name>.stl and print its size."""
    out.mkdir(exist_ok=True)
    print(
        f"box {OUT:.1f} x {OUT:.1f} x {DEPTH:.1f}, tilted {TILT:.0f} deg off vertical"
    )
    for name, part in PRINTED.items():
        export_stl(part, str(out / f"{name}.stl"))
        b = part.bounding_box()
        print(
            f"{name:9s} {b.size.X:5.1f} x {b.size.Y:5.1f} x {b.size.Z:5.1f}"
            f"  {part.volume / 1000:6.1f} cm3  -> {out.name}/{name}.stl"
        )


def write_step(path):
    """Write every part, named and coloured, into a single STEP assembly.

    Layout: box / {printed, reference} / <part>, all in assembled position,
    colours as in the browser viewer (STEP keeps no opacity).
    """
    groups = []
    for label, parts in (("printed", PRINTED), ("reference", REFERENCE)):
        children = []
        for name, part in parts.items():
            solid = part.moved(Location())  # a copy: labels stick to the shape
            solid.label = name
            solid.color = Color(int(LOOK[name][0][1:], 16))
            children.append(solid)
        groups.append(Compound(children=children, label=label))
    export_step(Compound(children=groups, label="box"), str(path))
    print(f"{len(PRINTED) + len(REFERENCE)} parts -> {path}")


def serve(host, port):
    """Serve every part as STL plus a three.js page that assembles them.

    Everything is rendered up front and held in memory, so the handler only
    looks paths up; nothing is written next to the script.

    The page is viewer.html with its placeholders filled in:
      __PARTS__  JSON {name: {printed, color, opacity, explode}}
      __HALF__   range of the section-cut slider, mm
      __INFO__   footer line with the box size and tilt
    It fetches each part as <name>.stl from the same server.
    """
    files = {}  # URL path -> (content type, body)
    with tempfile.TemporaryDirectory() as tmp:  # export_stl wants a real file
        for name, part in {**PRINTED, **REFERENCE}.items():
            path = Path(tmp) / f"{name}.stl"
            export_stl(part, str(path))
            files[f"/{name}.stl"] = ("model/stl", path.read_bytes())

    # The page gets each part's look and role as JSON, plus a few numbers.
    parts = {
        name: {"printed": name in PRINTED, "color": c, "opacity": o, "explode": e}
        for name, (c, o, e) in LOOK.items()
    }
    info = f"{OUT:.1f} x {OUT:.1f} x {DEPTH:.1f} mm, tilt {TILT:.0f} deg"
    page = (
        (HERE / "viewer.html")
        .read_text()
        .replace("__PARTS__", json.dumps(parts))
        .replace("__HALF__", f"{HALF + 5:.0f}")
        .replace("__INFO__", info)
    )
    files["/"] = files["/index.html"] = ("text/html; charset=utf-8", page.encode())

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            hit = files.get(self.path.split("?")[0])
            if hit is None:
                self.send_error(404)
                return
            kind, body = hit
            self.send_response(200)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def log_message(self, *args):  # keep the terminal quiet
            pass

    with ThreadingHTTPServer((host, port), Handler) as httpd:
        print(f"viewer on http://{host}:{port}/  (ctrl-c to stop)", flush=True)
        try:
            httpd.serve_forever()
        except KeyboardInterrupt:
            pass


def show():
    """Hand every part to the OCP viewer (VS Code extension or standalone)."""
    try:
        from ocp_vscode import show_all

        show_all(globals())  # the parts live at module level, not in here
    except Exception as exc:  # headless: nothing to show
        print(f"viewer unavailable ({exc}); see --help for exports", file=sys.stderr)


if __name__ == "__main__":
    if ARGS.serve:
        serve(*ARGS.serve)
    elif ARGS.step:
        write_step(ARGS.step)
    elif ARGS.stl:
        write_stl(HERE / "stl")
    else:
        show()
