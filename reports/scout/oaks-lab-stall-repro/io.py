import sys
from gbemu.backend import GbEmuEmulator
emu = GbEmuEmulator(); emu.load(sys.argv[1]); emu.load_state(sys.argv[2])
print("TIMA TMA TAC", [hex(emu.read_u8(a)) for a in (0xFF05,0xFF06,0xFF07)], "IE", hex(emu.read_u8(0xFFFF)), "IF", hex(emu.read_u8(0xFF0F)), "STAT", hex(emu.read_u8(0xFF41)), "LY", emu.read_u8(0xFF44))
