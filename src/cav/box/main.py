"""Enclosure for an 8x8 WS2812 LED matrix driven from a breadboard.

The screen face is tilted 30 deg from horizontal (slope along Y), so the body
is a wedge: low at the front (-Y), high at the back (+Y). Multiple cases are
meant to sit side by side along X, so the left/right (X) walls are kept thin
and the matrix is pushed to the X edges for near-continuous tiling; all the
fasteners live on the Y sides where there is room.

Parts (shown together, modelled separately):
  * case       - the wedge tray
  * matrix     - the 65x65 WS2812 panel (edge mount holes on its +/-Y sides)
  * breadboard - 81x54x9 breadboard (long side along Y) with dovetail connectors
  * cover      - diffuser plate seated in the sloped face

The matrix-face features are built "flat" (face = XY plane, outward = +Z,
interior = -Z) and then moved onto the sloped face by TF. All sizes are mm.
"""

import math

from build123d import *
from ocp_vscode import *

C, MIN = Align.CENTER, Align.MIN

# --- Tilt ------------------------------------------------------------------
TILT = 30.0
ROOF = 3.0             # min material between the cavity ceiling and the face

# --- Matrix ----------------------------------------------------------------
MATRIX = 65.0
MATRIX_PCB = 1.6
MATRIX_TH = 4.0
MATRIX_LIP = 2.5
HOLE_D = 2.0           # Ø2 edge holes: three per side on the +/-Y sides
HOLE_PITCH = 6.0
HOLE_EDGE = 1.0

# --- Shell -----------------------------------------------------------------
WALL = 2.4             # front/back (Y) and vertical walls
WALL_X = 1.6           # thin left/right walls for side-by-side tiling
FLOOR = 2.0
BAY_H = 9.0            # breadboard height
EXTRA_H = 20.0         # extra headroom above the breadboard (dev boards + wires)

# --- Cover (diffuser) ------------------------------------------------------
COVER_CLR = 0.3
COVER_TH = 2.0
COV_Y = 78.0           # reaches the Y-side screw tabs

# --- Fasteners (cover screws on the Y margins, clear of the matrix) --------
SCREW_R = 1.0
SCREW_X = 22.0
SCREW_Y = 37.0
USB_W = 12.0
USB_H = 7.0
MOUNT_PILOT_R = 0.8    # self-tap pilot for the matrix edge screws

# --- Breadboard (long side along Y; connectors on the +/-X long sides) -----
BB_L = 81.0            # along Y
BB_W = 54.0            # along X
BB_H = 9.0
BB_CLR = 0.8
CONN_PRO = 3.0
CONN_NECK = 2.5
CONN_HEAD = 4.0        # largest side of the breadboard mount
CONN_SPACING = 40.0    # along Y
CONN_CLR = 0.3

# ---------------------------------------------------------------------------
# Derived geometry
# ---------------------------------------------------------------------------
pocket = MATRIX + 1.0
window = MATRIX - 2 * MATRIX_LIP
COV_X = pocket                      # cover spans to the thin X walls

outer_x = pocket + 2 * WALL_X
inner_x = outer_x - 2 * WALL_X      # == pocket; matrix pushed to the X edges
inner_y = BB_L + BB_CLR
outer_y = inner_y + 2 * WALL

z_ceil = FLOOR + BAY_H + EXTRA_H
Z_FACE = z_ceil + ROOF + (outer_y / 2) * math.tan(math.radians(TILT))
HTALL = 2 * Z_FACE + outer_y        # tall blank, trimmed to the sloped face

TF = Pos(0, 0, Z_FACE) * Rotation(-TILT, 0, 0)

z_cover_in = -COVER_TH
z_pocket_in = -(COVER_TH + MATRIX_TH)
WIN_DEEP = 50.0


def fbox(sx, sy, z0, z1, cx=0.0, cy=0.0):
    return Pos(cx, cy, z0) * Box(sx, sy, z1 - z0, align=(C, C, MIN))


def _dovetail_x(cy, edge_x, direction, z0, h, grow=0.0):
    """Trapezoid connector along X: narrow at edge_x, wide CONN_PRO out."""
    neck = CONN_NECK + 2 * grow
    head = CONN_HEAD + 2 * grow
    x0 = edge_x - direction * grow
    x1 = edge_x + direction * (CONN_PRO + grow)
    pts = [(x0, cy - neck / 2), (x1, cy - head / 2),
           (x1, cy + head / 2), (x0, cy + neck / 2)]
    return Pos(0, 0, z0) * extrude(Polygon(*pts, align=None), amount=h)


# Matrix edge holes on the +/-X sides (three each, centred, 6 mm pitch).
hole_xs = (MATRIX / 2 - HOLE_EDGE, -(MATRIX / 2 - HOLE_EDGE))
hole_ys = (-HOLE_PITCH, 0.0, HOLE_PITCH)
mount_pts = [(hx, hy) for hx in hole_xs for hy in hole_ys]
screw_pts = [(sx, sy) for sx in (SCREW_X, -SCREW_X) for sy in (SCREW_Y, -SCREW_Y)]

# ---------------------------------------------------------------------------
# Matrix panel
# ---------------------------------------------------------------------------
matrix_flat = fbox(MATRIX, MATRIX, z_pocket_in, z_pocket_in + MATRIX_PCB)
for hx, hy in mount_pts:
    matrix_flat -= Pos(hx, hy, z_pocket_in - 1) * Cylinder(HOLE_D / 2, MATRIX_PCB + 2, align=(C, C, MIN))
matrix = TF * matrix_flat

# ---------------------------------------------------------------------------
# Breadboard: males on +X long side, females on -X long side.
# ---------------------------------------------------------------------------
breadboard = fbox(BB_W, BB_L, FLOOR, FLOOR + BB_H)
conn_ys = (-CONN_SPACING / 2, CONN_SPACING / 2)
for cyp in conn_ys:
    breadboard += _dovetail_x(cyp, BB_W / 2, +1, FLOOR, BB_H)
    breadboard -= _dovetail_x(cyp, -BB_W / 2, +1, FLOOR, BB_H)

# ---------------------------------------------------------------------------
# Case body
# ---------------------------------------------------------------------------
case = fbox(outer_x, outer_y, 0, HTALL)
case -= TF * fbox(4 * outer_x, 4 * outer_y, 0, HTALL)          # trim to the face

case -= TF * fbox(COV_X, COV_Y, z_cover_in, 0)                # cover rebate
case -= TF * fbox(pocket, pocket, z_pocket_in, z_cover_in)    # matrix pocket
# Light/wiring window, clipped to the interior so it never breaches a wall.
case -= (TF * fbox(window, window, z_pocket_in - WIN_DEEP, z_pocket_in)) & fbox(
    inner_x, inner_y, FLOOR, HTALL
)

for hx, hy in mount_pts:                                      # matrix screw pilots
    case -= TF * (Pos(hx, hy, z_pocket_in - 5) * Cylinder(MOUNT_PILOT_R, 5.1, align=(C, C, MIN)))
for sx, sy in screw_pts:                                      # cover screw pilots
    case -= TF * (Pos(sx, sy, -12) * Cylinder(SCREW_R, 13, align=(C, C, MIN)))

case -= fbox(inner_x, inner_y, FLOOR, z_ceil)                 # breadboard cavity
# USB slot on the back (+Y, high) wall, up in the dev-board headroom.
usb_z = FLOOR + BAY_H + 4.0
case -= fbox(USB_W, 2 * WALL, usb_z, usb_z + USB_H, cy=inner_y / 2)

# Floor dovetail mates holding the breadboard by its connectors (on +/-X).
MATE_BASE = CONN_PRO + 3.0
for cyp in conn_ys:
    blk = fbox(MATE_BASE, CONN_HEAD + 6, 0, FLOOR + BB_H, cx=BB_W / 2 + MATE_BASE / 2, cy=cyp)
    blk -= _dovetail_x(cyp, BB_W / 2, +1, FLOOR, BB_H, grow=CONN_CLR)
    case += blk
    case += fbox(2.0, CONN_NECK + 4, 0, FLOOR + BB_H, cx=-BB_W / 2 - 1.0, cy=cyp)
    case += _dovetail_x(cyp, -BB_W / 2, +1, 0, FLOOR + BB_H, grow=-CONN_CLR)

# ---------------------------------------------------------------------------
# Diffuser cover
# ---------------------------------------------------------------------------
cover_flat = fbox(COV_X - 2 * COVER_CLR, COV_Y - 2 * COVER_CLR, z_cover_in, 0)
for sx, sy in screw_pts:
    cover_flat -= Pos(sx, sy, z_cover_in - 1) * Cylinder(SCREW_R + 0.2, COVER_TH + 2, align=(C, C, MIN))
cover = TF * cover_flat

show_all()
