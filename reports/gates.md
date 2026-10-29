# gates

**G1 12/12 G2 1/1 G3 4/4 G4 1/1 MOONEYE 94/94 REPLAY ok**

options: `--baseline reports/baseline.json`

| group | rom | result | frames | reason |
| --- | --- | --- | --- | --- |
| G1 | `blargg/cpu_instrs/individual/01-special.gb` | PASS | 140 |  |
| G1 | `blargg/cpu_instrs/individual/02-interrupts.gb` | PASS | 26 |  |
| G1 | `blargg/cpu_instrs/individual/03-op sp,hl.gb` | PASS | 141 |  |
| G1 | `blargg/cpu_instrs/individual/04-op r,imm.gb` | PASS | 164 |  |
| G1 | `blargg/cpu_instrs/individual/05-op rp.gb` | PASS | 225 |  |
| G1 | `blargg/cpu_instrs/individual/06-ld r,r.gb` | PASS | 35 |  |
| G1 | `blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb` | PASS | 43 |  |
| G1 | `blargg/cpu_instrs/individual/08-misc instrs.gb` | PASS | 33 |  |
| G1 | `blargg/cpu_instrs/individual/09-op r,r.gb` | PASS | 546 |  |
| G1 | `blargg/cpu_instrs/individual/10-bit ops.gb` | PASS | 829 |  |
| G1 | `blargg/cpu_instrs/individual/11-op a,(hl).gb` | PASS | 1051 |  |
| G1 | `blargg/cpu_instrs/cpu_instrs.gb` | PASS | 3196 |  |
| G2 | `blargg/instr_timing/instr_timing.gb` | PASS | 39 |  |
| G3 | `blargg/mem_timing/mem_timing.gb` | PASS | 91 |  |
| G3 | `blargg/mem_timing/individual/01-read_timing.gb` | PASS | 28 |  |
| G3 | `blargg/mem_timing/individual/02-write_timing.gb` | PASS | 27 |  |
| G3 | `blargg/mem_timing/individual/03-modify_timing.gb` | PASS | 30 |  |
| G4 | `acid2/dmg-acid2.gb` | PASS | 20 |  |

## Mooneye

scored (DMG-ABC scope, see `mooneye-scope.md`): **94/94**; raw (all ROMs): **95/115**

| rom | in scope | result | frames | reason |
| --- | --- | --- | --- | --- |
| `acceptance/add_sp_e_timing` | yes | PASS | 13 |  |
| `acceptance/bits/mem_oam` | yes | PASS | 10 |  |
| `acceptance/bits/reg_f` | yes | PASS | 10 |  |
| `acceptance/bits/unused_hwio-GS` | yes | PASS | 10 |  |
| `acceptance/boot_div-S` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_div-dmg0` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_div-dmgABCmgb` | yes | PASS | 10 |  |
| `acceptance/boot_div2-S` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_hwio-S` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_hwio-dmg0` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_hwio-dmgABCmgb` | yes | PASS | 10 |  |
| `acceptance/boot_regs-dmg0` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_regs-dmgABC` | yes | PASS | 10 |  |
| `acceptance/boot_regs-mgb` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_regs-sgb` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/boot_regs-sgb2` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `acceptance/call_cc_timing` | yes | PASS | 13 |  |
| `acceptance/call_cc_timing2` | yes | PASS | 14 |  |
| `acceptance/call_timing` | yes | PASS | 13 |  |
| `acceptance/call_timing2` | yes | PASS | 14 |  |
| `acceptance/di_timing-GS` | yes | PASS | 15 |  |
| `acceptance/div_timing` | yes | PASS | 10 |  |
| `acceptance/ei_sequence` | yes | PASS | 10 |  |
| `acceptance/ei_timing` | yes | PASS | 10 |  |
| `acceptance/halt_ime0_ei` | yes | PASS | 10 |  |
| `acceptance/halt_ime0_nointr_timing` | yes | PASS | 14 |  |
| `acceptance/halt_ime1_timing` | yes | PASS | 10 |  |
| `acceptance/halt_ime1_timing2-GS` | yes | PASS | 18 |  |
| `acceptance/if_ie_registers` | yes | PASS | 10 |  |
| `acceptance/instr/daa` | yes | PASS | 22 |  |
| `acceptance/interrupts/ie_push` | yes | PASS | 10 |  |
| `acceptance/intr_timing` | yes | PASS | 10 |  |
| `acceptance/jp_cc_timing` | yes | PASS | 13 |  |
| `acceptance/jp_timing` | yes | PASS | 13 |  |
| `acceptance/ld_hl_sp_e_timing` | yes | PASS | 13 |  |
| `acceptance/oam_dma/basic` | yes | PASS | 10 |  |
| `acceptance/oam_dma/reg_read` | yes | PASS | 10 |  |
| `acceptance/oam_dma/sources-GS` | yes | PASS | 36 |  |
| `acceptance/oam_dma_restart` | yes | PASS | 13 |  |
| `acceptance/oam_dma_start` | yes | PASS | 14 |  |
| `acceptance/oam_dma_timing` | yes | PASS | 13 |  |
| `acceptance/pop_timing` | yes | PASS | 10 |  |
| `acceptance/ppu/hblank_ly_scx_timing-GS` | yes | PASS | 47 |  |
| `acceptance/ppu/intr_1_2_timing-GS` | yes | PASS | 13 |  |
| `acceptance/ppu/intr_2_0_timing` | yes | PASS | 12 |  |
| `acceptance/ppu/intr_2_mode0_timing` | yes | PASS | 12 |  |
| `acceptance/ppu/intr_2_mode0_timing_sprites` | yes | PASS | 238 |  |
| `acceptance/ppu/intr_2_mode3_timing` | yes | PASS | 12 |  |
| `acceptance/ppu/intr_2_oam_ok_timing` | yes | PASS | 12 |  |
| `acceptance/ppu/lcdon_timing-GS` | yes | PASS | 30 |  |
| `acceptance/ppu/lcdon_write_timing-GS` | yes | PASS | 55 |  |
| `acceptance/ppu/stat_irq_blocking` | yes | PASS | 11 |  |
| `acceptance/ppu/stat_lyc_onoff` | yes | PASS | 13 |  |
| `acceptance/ppu/vblank_stat_intr-GS` | yes | PASS | 18 |  |
| `acceptance/push_timing` | yes | PASS | 13 |  |
| `acceptance/rapid_di_ei` | yes | PASS | 10 |  |
| `acceptance/ret_cc_timing` | yes | PASS | 14 |  |
| `acceptance/ret_timing` | yes | PASS | 14 |  |
| `acceptance/reti_intr_timing` | yes | PASS | 10 |  |
| `acceptance/reti_timing` | yes | PASS | 14 |  |
| `acceptance/rst_timing` | yes | PASS | 13 |  |
| `acceptance/serial/boot_sclk_align-dmgABCmgb` | yes | PASS | 10 |  |
| `acceptance/timer/div_write` | yes | PASS | 51 |  |
| `acceptance/timer/rapid_toggle` | yes | PASS | 10 |  |
| `acceptance/timer/tim00` | yes | PASS | 10 |  |
| `acceptance/timer/tim00_div_trigger` | yes | PASS | 10 |  |
| `acceptance/timer/tim01` | yes | PASS | 10 |  |
| `acceptance/timer/tim01_div_trigger` | yes | PASS | 10 |  |
| `acceptance/timer/tim10` | yes | PASS | 10 |  |
| `acceptance/timer/tim10_div_trigger` | yes | PASS | 10 |  |
| `acceptance/timer/tim11` | yes | PASS | 10 |  |
| `acceptance/timer/tim11_div_trigger` | yes | PASS | 10 |  |
| `acceptance/timer/tima_reload` | yes | PASS | 10 |  |
| `acceptance/timer/tima_write_reloading` | yes | PASS | 10 |  |
| `acceptance/timer/tma_write_reloading` | yes | PASS | 10 |  |
| `emulator-only/mbc1/bits_bank1` | yes | PASS | 178 |  |
| `emulator-only/mbc1/bits_bank2` | yes | PASS | 175 |  |
| `emulator-only/mbc1/bits_mode` | yes | PASS | 178 |  |
| `emulator-only/mbc1/bits_ramg` | yes | PASS | 348 |  |
| `emulator-only/mbc1/multicart_rom_8Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc1/ram_256kb` | yes | PASS | 55 |  |
| `emulator-only/mbc1/ram_64kb` | yes | PASS | 55 |  |
| `emulator-only/mbc1/rom_16Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc1/rom_1Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc1/rom_2Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc1/rom_4Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc1/rom_512kb` | yes | PASS | 13 |  |
| `emulator-only/mbc1/rom_8Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc2/bits_ramg` | yes | PASS | 355 |  |
| `emulator-only/mbc2/bits_romb` | yes | PASS | 186 |  |
| `emulator-only/mbc2/bits_unused` | yes | PASS | 174 |  |
| `emulator-only/mbc2/ram` | yes | PASS | 28 |  |
| `emulator-only/mbc2/rom_1Mb` | yes | PASS | 10 |  |
| `emulator-only/mbc2/rom_2Mb` | yes | PASS | 10 |  |
| `emulator-only/mbc2/rom_512kb` | yes | PASS | 10 |  |
| `emulator-only/mbc5/rom_16Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc5/rom_1Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc5/rom_2Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc5/rom_32Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc5/rom_4Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc5/rom_512kb` | yes | PASS | 13 |  |
| `emulator-only/mbc5/rom_64Mb` | yes | PASS | 13 |  |
| `emulator-only/mbc5/rom_8Mb` | yes | PASS | 13 |  |
| `madness/mgb_oam_dma_halt_sprites` | no | FAIL | 1792 | timeout: no LD B,B within 1792 frames |
| `manual-only/sprite_priority` | no | FAIL | 10 | unexpected registers at LD B,B (B=00 C=00 D=02 E=2E H=FF L=40) |
| `misc/bits/unused_hwio-C` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `misc/boot_div-A` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `misc/boot_div-cgb0` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `misc/boot_div-cgbABCDE` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `misc/boot_hwio-C` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `misc/boot_regs-A` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `misc/boot_regs-cgb` | no | FAIL | 10 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `misc/ppu/vblank_stat_intr-C` | no | FAIL | 18 | test reported failure (B=42 C=42 D=42 E=42 H=42 L=42) |
| `utils/bootrom_dumper` | no | FAIL | 50 | unexpected registers at LD B,B (B=00 C=00 D=09 E=00 H=FF L=40) |
| `utils/dump_boot_hwio` | no | PASS | 10 |  |

logs: `logs/<rom>.log`; screenshots: `blargg/<rom>.png`, `acid2-actual.png`, `acid2-diff.png` (on failure).
