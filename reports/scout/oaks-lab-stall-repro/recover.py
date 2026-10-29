"""Reach the W4/T62 stall (wait k, press A, idle 150), then probe inputs and START-menu recovery."""
import sys
from gbemu.backend import GbEmuEmulator
from gbemu.replay import apply_action
rom, state, k = sys.argv[1], sys.argv[2], int(sys.argv[3])
emu = GbEmuEmulator(); emu.load(rom); emu.load_state(state); R = emu.read_u8
emu.tick(k); emu.press("a", 8); emu.tick(150)
def s(t):
    print(f"{t:12s} xy=({R(0xD362)},{R(0xD361)}) scr={R(0xD5F0)} c102={R(0xC102):02x} facing(c109)={R(0xC109):02x} t89={R(0xC45C):02x} t87={R(0xC3A0+7*20+8):02x} "
          f"t69={R(0xC3A0+9*20+6):02x} t109={R(0xC3A0+9*20+10):02x} front(cfc6)={R(0xCFC6):02x} coll(c10c)={R(0xC10C):02x} pdir={R(0xD52A):02x} pmd={R(0xD528):02x} wc={R(0xCFC5)} joyIgn={R(0xCD6B):02x} sf5={R(0xD730):02x} c200={R(0xC200):02x}")
s("stalled")
for a in ["walk_left", "walk_right", "walk_down", "walk_up", "press_a", "press_b"]:
    apply_action(emu, a); s(a)
emu.get_screen().save("/tmp/gbemu-scout/stall-W4.png")
for a in ["press_start", "wait_30", "press_b", "wait_60"]:
    apply_action(emu, a); s(a)
emu.get_screen().save("/tmp/gbemu-scout/after-start.png")
for a in ["walk_left", "walk_down", "walk_down"]:
    apply_action(emu, a); s(a)
