import json, sys
from gbemu.backend import GbEmuEmulator
from gbemu.replay import apply_action
log, rom, upto, out = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
emu = GbEmuEmulator(); emu.load(rom); emu.tick(60)
reqs = [json.loads(l) for l in open(log)]
for r in reqs[:upto]:
    for a in r.get("actions") or []: apply_action(emu, a)
emu.save_state(out)
print("saved", out, emu.frame_count, emu.read_u8(0xD35E), emu.read_u8(0xD362), emu.read_u8(0xD361))
