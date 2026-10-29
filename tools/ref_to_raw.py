"""T01: convert the dmg-acid2 reference PNG into raw 8-bit grayscale (23040 bytes).

Usage: .venv/bin/python tools/ref_to_raw.py [REFS]   (REFS defaults to $GBEMU_REFS or ./refs)
The CLI then compares frames byte-for-byte without decoding PNG.
"""
import hashlib
import os
import sys
from pathlib import Path

from PIL import Image

EXPECTED_SHA256 = "cb26ef6174cd5b61662c2e74f2bdb2b0f8da0750234c75144c3a4d258fda8347"

refs = Path(sys.argv[1] if len(sys.argv) > 1 else os.environ.get("GBEMU_REFS", "refs"))
src = refs / "acid2" / "reference-dmg.png"
dst = refs / "acid2" / "reference-dmg.gray"
img = Image.open(src)
assert img.size == (160, 144), img.size
raw = img.convert("L").tobytes()
assert len(raw) == 23040, len(raw)
assert set(raw) <= {0x00, 0x55, 0xAA, 0xFF}, sorted(set(raw))
digest = hashlib.sha256(raw).hexdigest()
assert digest == EXPECTED_SHA256, digest
dst.write_bytes(raw)
print(f"OK {dst} sha256={digest} values={sorted(set(raw))}")
