import json, sys
from gbemu.backend import GbEmuEmulator
from gbemu.replay import apply_action
log, rom, state, start, end, png = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), int(sys.argv[5]), sys.argv[6]
emu = GbEmuEmulator(); emu.load(rom); emu.load_state(state)
reqs = [json.loads(l) for l in open(log)]
for i in range(start, end):
    for a in reqs[i].get("actions") or []: apply_action(emu, a)
def ch(t):
    if 0x80 <= t <= 0x99: return chr(ord('A') + t - 0x80)
    if 0xA0 <= t <= 0xB9: return chr(ord('a') + t - 0xA0)
    if 0xF6 <= t <= 0xFF: return chr(ord('0') + t - 0xF6)
    if t == 0x7F: return ' '
    return None
tm = emu.read_range(0xC3A0, 360)
for y in range(18):
    row = tm[y*20:(y+1)*20]
    print("%2d " % y + " ".join("%02x" % b for b in row) + "   " + "".join((ch(b) or '.') for b in row))
emu.get_screen().save(png)
print("wd730", hex(emu.read_u8(0xD730)), "hWY", hex(emu.read_u8(0xFFB0)), "WY", hex(emu.read_u8(0xFF4A)), "autoBG", hex(emu.read_u8(0xFFBA)))
