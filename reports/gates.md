# gates

**G1 12/12 G2 1/1 G3 4/4 G4 0/1**

options: `--baseline reports/baseline.json`

| group | rom | result | frames | reason |
| --- | --- | --- | --- | --- |
| G1 | `blargg/cpu_instrs/individual/01-special.gb` | PASS | 143 |  |
| G1 | `blargg/cpu_instrs/individual/02-interrupts.gb` | PASS | 27 |  |
| G1 | `blargg/cpu_instrs/individual/03-op sp,hl.gb` | PASS | 143 |  |
| G1 | `blargg/cpu_instrs/individual/04-op r,imm.gb` | PASS | 167 |  |
| G1 | `blargg/cpu_instrs/individual/05-op rp.gb` | PASS | 227 |  |
| G1 | `blargg/cpu_instrs/individual/06-ld r,r.gb` | PASS | 37 |  |
| G1 | `blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb` | PASS | 45 |  |
| G1 | `blargg/cpu_instrs/individual/08-misc instrs.gb` | PASS | 35 |  |
| G1 | `blargg/cpu_instrs/individual/09-op r,r.gb` | PASS | 548 |  |
| G1 | `blargg/cpu_instrs/individual/10-bit ops.gb` | PASS | 832 |  |
| G1 | `blargg/cpu_instrs/individual/11-op a,(hl).gb` | PASS | 1053 |  |
| G1 | `blargg/cpu_instrs/cpu_instrs.gb` | PASS | 3208 |  |
| G2 | `blargg/instr_timing/instr_timing.gb` | PASS | 41 |  |
| G3 | `blargg/mem_timing/mem_timing.gb` | PASS | 95 |  |
| G3 | `blargg/mem_timing/individual/01-read_timing.gb` | PASS | 30 |  |
| G3 | `blargg/mem_timing/individual/02-write_timing.gb` | PASS | 29 |  |
| G3 | `blargg/mem_timing/individual/03-modify_timing.gb` | PASS | 32 |  |
| G4 | `acid2/dmg-acid2.gb` | FAIL | 119 | no LD B,B within 119 frames; 10191 px differ, bbox (13,0)-(147,141) |

logs: `logs/<rom>.log`; screenshots: `blargg/<rom>.png`, `acid2-actual.png`, `acid2-diff.png` (on failure).
