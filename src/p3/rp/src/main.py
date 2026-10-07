#!/usr/bin/env python3
"""
cava_matrix.py - stream a CAVA audio visualizer to a square LED matrix.

    python3 cava_matrix.py 60 /dev/ttyACM0

1. Sends the SIZE command and reads back the number of pixels (square display).
2. Writes a CAVA config (one bar per column, the given FPS) and starts CAVA.
3. For every CAVA frame sends FRAME_CMD followed by pixels*3 bytes of RGB,
   row by row from the top-left (row 0 = top, left to right).

Requires: pip install pyserial, and cava on the PATH.
"""
import argparse
import math
import os
import subprocess
import sys
import tempfile
import threading
import time

import serial

# ---- Protocol: adjust to match your firmware ---------------------------------
SIZE_CMD = b"size\r"          # firmware replies with the pixel count, e.g. "64\r\n"
FRAME_CMD = b"frame\r"        # followed by exactly pixels*3 raw RGB bytes
SIZE_TIMEOUT = 2.0            # seconds to wait for the SIZE reply

# ---- Look & feel ---------------------------------------------------------------
BRIGHTNESS = 0.35             # global cap 0..1 (also limits current draw)
GAMMA = 2.2                   # perceptual correction for WS2812
PEAK_HOLD_S = 0.2             # seconds a peak dot stays before falling
PEAK_FALL = 1.2               # peak fall speed, in display heights per second
PEAK_COLOR = (0.7, 0.7, 0.9)


def query_size(ser):
    """Send SIZE and return the first all-digit line of the reply.
    Other lines (e.g. the firmware echoing the command) are ignored."""
    ser.reset_input_buffer()
    ser.write(SIZE_CMD)
    deadline = time.monotonic() + SIZE_TIMEOUT
    buf = b""
    while time.monotonic() < deadline:
        buf += ser.read(ser.in_waiting or 1)
        *lines, buf = buf.replace(b"\r", b"\n").split(b"\n")
        for line in lines:
            if line.strip().isdigit():
                return int(line.strip())
    sys.exit(f"error: no reply to {SIZE_CMD!r} within {SIZE_TIMEOUT} s")


def drain(ser, stop):
    """Keep reading whatever the device sends (echo, messages). If nobody reads,
    the device's USB writes eventually block and it stops accepting frames."""
    while not stop.is_set():
        try:
            ser.read(ser.in_waiting or 1)
        except (serial.SerialException, OSError, TypeError):
            return


def cava_config(bars, fps):
    return f"""[general]
bars = {bars}
framerate = {fps}

[output]
method = raw
raw_target = /dev/stdout
data_format = binary
bit_format = 8bit
channels = mono
mono_option = average
"""


def read_exact(stream, n):
    """Read exactly n bytes from an unbuffered pipe; None on EOF."""
    data = b""
    while len(data) < n:
        chunk = stream.read(n - len(data))
        if not chunk:
            return None
        data += chunk
    return data


class Renderer:
    """Turns `side` bar heights (0..255) into a side x side RGB frame."""

    def __init__(self, side, fps):
        self.side = side
        self.hold_frames = round(PEAK_HOLD_S * fps)
        self.fall = PEAK_FALL * side / fps        # rows per frame
        self.peaks = [0.0] * side
        self.hold = [0] * side
        # Row colours from the bottom up: green -> yellow -> red
        self.rows = []
        for y in range(side):
            t = y / (side - 1) if side > 1 else 0.0
            self.rows.append((min(1.0, 2 * t), min(1.0, 2 * (1 - t)), 0.0))

    @staticmethod
    def color(rgb, level=1.0):
        return bytes(int(255 * BRIGHTNESS * (c * level) ** GAMMA + 0.5) for c in rgb)

    def put(self, frame, x, y, rgb):
        """y = 0 is the bottom row; the frame itself is sent top row first."""
        off = ((self.side - 1 - y) * self.side + x) * 3
        frame[off:off + 3] = rgb

    def render(self, bars):
        s = self.side
        frame = bytearray(s * s * 3)
        for x, v in enumerate(bars):
            h = v / 255 * s                        # bar height in rows

            # Bar body; the top pixel is dimmed by the fractional part.
            for y in range(min(s, math.ceil(h))):
                self.put(frame, x, y, self.color(self.rows[y], min(1.0, h - y)))

            # Peak dot with hold time and gravity.
            if h >= self.peaks[x]:
                self.peaks[x], self.hold[x] = h, self.hold_frames
            elif self.hold[x]:
                self.hold[x] -= 1
            else:
                self.peaks[x] = max(0.0, self.peaks[x] - self.fall)

            py = min(int(self.peaks[x]), s - 1)
            if self.peaks[x] >= 1 and py >= math.ceil(h):
                self.put(frame, x, py, self.color(PEAK_COLOR))
        return bytes(frame)


def main():
    ap = argparse.ArgumentParser(description="Stream CAVA to a square LED matrix over serial.")
    ap.add_argument("fps", type=int, help="frames per second, e.g. 60")
    ap.add_argument("device", help="serial device, e.g. /dev/ttyACM0")
    args = ap.parse_args()
    if not 1 <= args.fps <= 240:
        sys.exit("error: fps must be between 1 and 240")

    ser = serial.Serial(args.device, timeout=0.1, write_timeout=2)

    pixels = query_size(ser)
    side = math.isqrt(pixels)
    if pixels == 0 or side * side != pixels:
        sys.exit(f"error: display reports {pixels} pixels, which is not a square")
    print(f"display {side}x{side} ({pixels} px), {args.fps} FPS", file=sys.stderr)

    return;

    stop = threading.Event()
    reader = threading.Thread(target=drain, args=(ser, stop), daemon=True)
    reader.start()

    with tempfile.NamedTemporaryFile("w", prefix="cava_matrix_", suffix=".conf", delete=False) as f:
        f.write(cava_config(side, args.fps))
        config_path = f.name

    try:
        cava = subprocess.Popen(["cava", "-p", config_path], stdout=subprocess.PIPE, bufsize=0)
    except FileNotFoundError:
        os.unlink(config_path)
        sys.exit("error: cava not found on PATH")

    renderer = Renderer(side, args.fps)
    blank = FRAME_CMD + bytes(pixels * 3)
    try:
        while True:
            bars = read_exact(cava.stdout, side)
            if bars is None:
                print("cava exited", file=sys.stderr)
                break
            ser.write(FRAME_CMD + renderer.render(bars))
    except KeyboardInterrupt:
        pass
    except serial.SerialTimeoutException:
        print("error: device stopped accepting data", file=sys.stderr)
    finally:
        cava.terminate()
        try:
            cava.wait(timeout=2)
        except subprocess.TimeoutExpired:
            cava.kill()
        os.unlink(config_path)
        try:
            ser.write(blank)                        # leave the matrix dark
            ser.flush()
        except (serial.SerialException, OSError):
            pass
        stop.set()
        reader.join(timeout=1)
        ser.close()


if __name__ == "__main__":
    main()
