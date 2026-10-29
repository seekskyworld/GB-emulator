"""From a state at request 4076 (OAK 'Don't go away' text on screen), wait k frames,
press A (8 frames), then idle 150 frames; report whether wShadowOAM overflowed into wTileMap."""
import sys
from gbemu.backend import GbEmuEmulator
rom, state, kmax = sys.argv[1], sys.argv[2], int(sys.argv[3])
hold_up = len(sys.argv) > 4
hits = []
for k in range(kmax):
    emu = GbEmuEmulator(); emu.load(rom); emu.load_state(state)
    R = emu.read_u8
    emu.tick(k)
    emu.press("a", 8)
    maxvis = 0; ovf = False
    for f in range(150):
        if hold_up: emu.press("up", 1)
        else: emu.tick(1)
        vis = sum(1 for i in range(16) if R(0xC100+i*16) and R(0xC102+i*16) != 0xff)
        maxvis = max(maxvis, vis)
        if R(0xFF90) > 0xA0: ovf = True
    girl = (R(0xC295)-4, R(0xC294)-4)
    st = "STALL" if (R(0xC45C) >= 0x60 and R(0xC102) == 0xff) else "ok"
    print(f"k={k:2d} xy=({R(0xD362)},{R(0xD361)}) scr={R(0xD5F0)} maxvis={maxvis} ovf={ovf} t89={R(0xC45C):02x} c102={R(0xC102):02x} girl={girl} {st}")
    if st == "STALL": hits.append(k)
    emu.close()
print("stall at k =", hits, f"({len(hits)}/{kmax})")
