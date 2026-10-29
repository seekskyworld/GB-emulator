# Mooneye scope (DMG-ABC)

Test suite: `$GBEMU_REFS/mooneye/mts-20260714-0944-31510e1`, 115 ROMs.

Rules (PLAN §2.4): only `acceptance/` and `emulator-only/`; the model suffix is the part of the file name after the last `-` and before `.gb`. Included: no suffix, a suffix containing `G` (dmg+mgb group, e.g. `-GS`), or a suffix containing `dmgABC` (e.g. `-dmgABC`, `-dmgABCmgb`). Everything else is excluded (`-dmg0`, `-mgb`, `-S`, `-sgb`, `-sgb2`, `-cgb*`, `-A`, `-C`, and the `manual-only/`, `madness/`, `utils/`, `misc/` directories).

Included: 94 / 115; passing: 94/94 (scored), 95/115 (raw).

## Included (94)

| rom | reason | result |
| --- | --- | --- |
| `acceptance/add_sp_e_timing` | no model suffix | PASS |
| `acceptance/bits/mem_oam` | no model suffix | PASS |
| `acceptance/bits/reg_f` | no model suffix | PASS |
| `acceptance/bits/unused_hwio-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/boot_div-dmgABCmgb` | suffix -dmgABCmgb contains dmgABC | PASS |
| `acceptance/boot_hwio-dmgABCmgb` | suffix -dmgABCmgb contains dmgABC | PASS |
| `acceptance/boot_regs-dmgABC` | suffix -dmgABC contains dmgABC | PASS |
| `acceptance/call_cc_timing` | no model suffix | PASS |
| `acceptance/call_cc_timing2` | no model suffix | PASS |
| `acceptance/call_timing` | no model suffix | PASS |
| `acceptance/call_timing2` | no model suffix | PASS |
| `acceptance/di_timing-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/div_timing` | no model suffix | PASS |
| `acceptance/ei_sequence` | no model suffix | PASS |
| `acceptance/ei_timing` | no model suffix | PASS |
| `acceptance/halt_ime0_ei` | no model suffix | PASS |
| `acceptance/halt_ime0_nointr_timing` | no model suffix | PASS |
| `acceptance/halt_ime1_timing` | no model suffix | PASS |
| `acceptance/halt_ime1_timing2-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/if_ie_registers` | no model suffix | PASS |
| `acceptance/instr/daa` | no model suffix | PASS |
| `acceptance/interrupts/ie_push` | no model suffix | PASS |
| `acceptance/intr_timing` | no model suffix | PASS |
| `acceptance/jp_cc_timing` | no model suffix | PASS |
| `acceptance/jp_timing` | no model suffix | PASS |
| `acceptance/ld_hl_sp_e_timing` | no model suffix | PASS |
| `acceptance/oam_dma/basic` | no model suffix | PASS |
| `acceptance/oam_dma/reg_read` | no model suffix | PASS |
| `acceptance/oam_dma/sources-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/oam_dma_restart` | no model suffix | PASS |
| `acceptance/oam_dma_start` | no model suffix | PASS |
| `acceptance/oam_dma_timing` | no model suffix | PASS |
| `acceptance/pop_timing` | no model suffix | PASS |
| `acceptance/ppu/hblank_ly_scx_timing-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/ppu/intr_1_2_timing-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/ppu/intr_2_0_timing` | no model suffix | PASS |
| `acceptance/ppu/intr_2_mode0_timing` | no model suffix | PASS |
| `acceptance/ppu/intr_2_mode0_timing_sprites` | no model suffix | PASS |
| `acceptance/ppu/intr_2_mode3_timing` | no model suffix | PASS |
| `acceptance/ppu/intr_2_oam_ok_timing` | no model suffix | PASS |
| `acceptance/ppu/lcdon_timing-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/ppu/lcdon_write_timing-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/ppu/stat_irq_blocking` | no model suffix | PASS |
| `acceptance/ppu/stat_lyc_onoff` | no model suffix | PASS |
| `acceptance/ppu/vblank_stat_intr-GS` | suffix -GS contains G (dmg+mgb) | PASS |
| `acceptance/push_timing` | no model suffix | PASS |
| `acceptance/rapid_di_ei` | no model suffix | PASS |
| `acceptance/ret_cc_timing` | no model suffix | PASS |
| `acceptance/ret_timing` | no model suffix | PASS |
| `acceptance/reti_intr_timing` | no model suffix | PASS |
| `acceptance/reti_timing` | no model suffix | PASS |
| `acceptance/rst_timing` | no model suffix | PASS |
| `acceptance/serial/boot_sclk_align-dmgABCmgb` | suffix -dmgABCmgb contains dmgABC | PASS |
| `acceptance/timer/div_write` | no model suffix | PASS |
| `acceptance/timer/rapid_toggle` | no model suffix | PASS |
| `acceptance/timer/tim00` | no model suffix | PASS |
| `acceptance/timer/tim00_div_trigger` | no model suffix | PASS |
| `acceptance/timer/tim01` | no model suffix | PASS |
| `acceptance/timer/tim01_div_trigger` | no model suffix | PASS |
| `acceptance/timer/tim10` | no model suffix | PASS |
| `acceptance/timer/tim10_div_trigger` | no model suffix | PASS |
| `acceptance/timer/tim11` | no model suffix | PASS |
| `acceptance/timer/tim11_div_trigger` | no model suffix | PASS |
| `acceptance/timer/tima_reload` | no model suffix | PASS |
| `acceptance/timer/tima_write_reloading` | no model suffix | PASS |
| `acceptance/timer/tma_write_reloading` | no model suffix | PASS |
| `emulator-only/mbc1/bits_bank1` | no model suffix | PASS |
| `emulator-only/mbc1/bits_bank2` | no model suffix | PASS |
| `emulator-only/mbc1/bits_mode` | no model suffix | PASS |
| `emulator-only/mbc1/bits_ramg` | no model suffix | PASS |
| `emulator-only/mbc1/multicart_rom_8Mb` | no model suffix | PASS |
| `emulator-only/mbc1/ram_256kb` | no model suffix | PASS |
| `emulator-only/mbc1/ram_64kb` | no model suffix | PASS |
| `emulator-only/mbc1/rom_16Mb` | no model suffix | PASS |
| `emulator-only/mbc1/rom_1Mb` | no model suffix | PASS |
| `emulator-only/mbc1/rom_2Mb` | no model suffix | PASS |
| `emulator-only/mbc1/rom_4Mb` | no model suffix | PASS |
| `emulator-only/mbc1/rom_512kb` | no model suffix | PASS |
| `emulator-only/mbc1/rom_8Mb` | no model suffix | PASS |
| `emulator-only/mbc2/bits_ramg` | no model suffix | PASS |
| `emulator-only/mbc2/bits_romb` | no model suffix | PASS |
| `emulator-only/mbc2/bits_unused` | no model suffix | PASS |
| `emulator-only/mbc2/ram` | no model suffix | PASS |
| `emulator-only/mbc2/rom_1Mb` | no model suffix | PASS |
| `emulator-only/mbc2/rom_2Mb` | no model suffix | PASS |
| `emulator-only/mbc2/rom_512kb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_16Mb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_1Mb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_2Mb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_32Mb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_4Mb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_512kb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_64Mb` | no model suffix | PASS |
| `emulator-only/mbc5/rom_8Mb` | no model suffix | PASS |

## Excluded (21)

| rom | reason | result |
| --- | --- | --- |
| `acceptance/boot_div-S` | suffix -S does not cover DMG-ABC | FAIL |
| `acceptance/boot_div-dmg0` | suffix -dmg0 does not cover DMG-ABC | FAIL |
| `acceptance/boot_div2-S` | suffix -S does not cover DMG-ABC | FAIL |
| `acceptance/boot_hwio-S` | suffix -S does not cover DMG-ABC | FAIL |
| `acceptance/boot_hwio-dmg0` | suffix -dmg0 does not cover DMG-ABC | FAIL |
| `acceptance/boot_regs-dmg0` | suffix -dmg0 does not cover DMG-ABC | FAIL |
| `acceptance/boot_regs-mgb` | suffix -mgb does not cover DMG-ABC | FAIL |
| `acceptance/boot_regs-sgb` | suffix -sgb does not cover DMG-ABC | FAIL |
| `acceptance/boot_regs-sgb2` | suffix -sgb2 does not cover DMG-ABC | FAIL |
| `madness/mgb_oam_dma_halt_sprites` | directory madness/ is not acceptance/ or emulator-only/ | FAIL |
| `manual-only/sprite_priority` | directory manual-only/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/bits/unused_hwio-C` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/boot_div-A` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/boot_div-cgb0` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/boot_div-cgbABCDE` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/boot_hwio-C` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/boot_regs-A` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/boot_regs-cgb` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `misc/ppu/vblank_stat_intr-C` | directory misc/ is not acceptance/ or emulator-only/ | FAIL |
| `utils/bootrom_dumper` | directory utils/ is not acceptance/ or emulator-only/ | FAIL |
| `utils/dump_boot_hwio` | directory utils/ is not acceptance/ or emulator-only/ | PASS |

