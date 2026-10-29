import json, sys
from gbemu.backend import GbEmuEmulator
log, rom, state, start, end = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5])
perframe = len(sys.argv) > 6
emu = GbEmuEmulator(); emu.load(rom); emu.load_state(state)
reqs = [json.loads(l) for l in open(log)]
R = emu.read_u8
def st(tag):
    r = emu.regs(); sp = r["sp"]
    stack = emu.read_range(sp, 12)
    sw = " ".join("%04x" % (stack[i] | stack[i+1] << 8) for i in range(0, 12, 2))
    print(f"{tag:22s} xy=({R(0xD362)},{R(0xD361)}) scr={R(0xD5F0):2d} simIdx={R(0xCD38):02x} simEnd={R(0xCCD3):02x} sf5(d730)={R(0xD730):02x} "
          f"joyIgn={R(0xCD6B):02x} walkCnt={R(0xCFC5):02x} pmd={R(0xD528):02x} pdir={R(0xD52A):02x} font={R(0xCFC4):02x} tb={R(0xD125):02x} "
          f"t89={R(0xC3A0+9*20+8):02x} c100={emu.read_range(0xC100,16).hex()} c200={emu.read_range(0xC200,16).hex()} "
          f"held={R(0xFFB4):02x} prs={R(0xFFB3):02x} in={R(0xFFF8):02x} dis={R(0xFFF9):02x} upd={R(0xCFCB):02x} misc={R(0xCD60):02x} "
          f"pc={r['pc']:04x} sp={sp:04x} stk=[{sw}] bank={R(0xFFB8):02x} ie={R(0xFFFF):02x} lcdc={R(0xFF40):02x}")
BTN = None
def run_action(a, tag):
    parts = a.split("_")
    if parts[0] in ("press", "walk"):
        seq = [("_".join(parts[1:]), 8), (None, 12)]
    elif parts[0] == "hold":
        seq = [("_".join(parts[1:-1]), int(parts[-1]))]
    else:
        seq = [(None, int(parts[1]))]
    for btn, n in seq:
        if perframe:
            for k in range(n):
                if btn: emu.press(btn, 1)
                else: emu.tick(1)
                st(f"{tag} {a} {btn or '-'}{k}")
        else:
            if btn: emu.press(btn, n)
            else: emu.tick(n)
    if not perframe: st(f"{tag} {a}")
for i in range(start, end):
    for a in reqs[i].get("actions") or []:
        run_action(a, f"r{i}")
