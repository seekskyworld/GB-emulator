# gates

**G1 12/12 G2 1/1 G3 4/4 G4 1/1**

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

logs: `logs/<rom>.log`; screenshots: `blargg/<rom>.png`, `acid2-actual.png`, `acid2-diff.png` (on failure).
