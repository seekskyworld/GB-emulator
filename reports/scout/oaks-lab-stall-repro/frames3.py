import json, sys
from gbemu.backend import GbEmuEmulator
log, rom, state, start, end = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
emu = GbEmuEmulator(); emu.load(rom); emu.load_state(state)
reqs = [json.loads(l) for l in open(log)]
R = emu.read_u8
def spr():
    s1 = emu.read_range(0xC100, 256); s2 = emu.read_range(0xC200, 256)
    out = []
    for i in range(16):
        p = s1[i*16]
        if p == 0: continue
        out.append(f"{i}:{s1[i*16+2]:02x}@({s2[i*16+5]-4},{s2[i*16+4]-4})y{s1[i*16+4]:02x}x{s1[i*16+6]:02x}m{s2[i*16+6]:02x}")
    return " ".join(out)
def st(tag):
    oamend = emu.read_range(0xC3A0, 16).hex()
    vis = sum(1 for i in range(16) if R(0xC100+i*16) and R(0xC102+i*16) != 0xff)
    r_=emu.regs(); print(f"{tag:18s} pc={r_['pc']:04x} bank={R(0xFFB8):02x} ly={R(0xFF44):3d} sp={r_['sp']:04x} hvb={R(0xFFD6):02x} cso={R(0xFFDA):02x} xy=({R(0xD362)},{R(0xD361)}) scr={R(0xD5F0)} sim={R(0xCD38):02x} sf5={R(0xD730):02x} wc={R(0xCFC5)} font={R(0xCFC4)} oamoff={R(0xFF90):02x} vis={vis} t89={R(0xC45C):02x} c3a0={oamend} lcdc={R(0xFF40):02x} hwy={R(0xFFB0):02x} | {spr()}")
f = 0
for i in range(start, end):
    for a in reqs[i].get("actions") or []:
        parts = a.split("_")
        if parts[0] in ("press", "walk"): seq = [("_".join(parts[1:]), 8), (None, 12)]
        elif parts[0] == "hold": seq = [("_".join(parts[1:-1]), int(parts[-1]))]
        else: seq = [(None, int(parts[1]))]
        for btn, n in seq:
            for k in range(n):
                if btn: emu.press(btn, 1)
                else: emu.tick(1)
                st(f"r{i} {a[:8]} {k}")
