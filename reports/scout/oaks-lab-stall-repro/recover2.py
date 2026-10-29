import sys
from gbemu.backend import GbEmuEmulator
from gbemu.replay import apply_action
emu = GbEmuEmulator(); emu.load(sys.argv[1]); emu.load_state(sys.argv[2]); R = emu.read_u8
def s(t): print(f"{t:12s} xy=({R(0xD362)},{R(0xD361)}) scr={R(0xD5F0)} c102={R(0xC102):02x} c109={R(0xC109):02x} t89={R(0xC45C):02x} t87={R(0xC3A0+7*20+8):02x} oamoff={R(0xFF90):02x} cfc6={R(0xCFC6):02x}")
s("at4080")
for a in ["walk_left","walk_down","press_start","wait_30","press_b","wait_60","walk_left","walk_right","walk_down","walk_down"]:
    apply_action(emu, a); s(a)
