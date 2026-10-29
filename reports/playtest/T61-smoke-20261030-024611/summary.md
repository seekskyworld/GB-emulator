# Playtest 20261030-024611

**Verdict: PASS** (exit code 0)

## Run
- driver: `script`
- minutes: `5.0`
- port: `8780`
- url: `http://localhost:8780`
- rom: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/roms/pokered.gb`
- rom_sha1: `ea9bcae617fdf159b045185467ae58b2e4a48b9a`
- gbemu_lib: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/play/target/release/libgb_ffi.dylib (pinned copy /Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/play/reports/playtest/20261030-024611/data/gbemu-lib/libgb_ffi.dylib)`
- gbemu_lib_sha256: `26e3bc035ed3ae347133a563e3c9cb5d724c056141955cfc9904807b0da9b294`
- git_head: `5a04c50`
- data_dir: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/play/reports/playtest/20261030-024611/data`
- started: `2026-10-30T02:46:11`
- ended: `2026-10-30T02:51:15`
- server_exit: `None`

## Hard checks

| check | result | detail |
| --- | --- | --- |
| samples taken | ok | 31 samples |
| server process alive throughout | ok |  |
| /health always responds | ok |  |
| /state always 200 with valid fields | ok |  |
| /screenshot always a 160x144 PNG | ok |  |
| frames advance after every /action | ok | 33723 ok /action, 0 without frame advance, 0 failed |
| /games/new called exactly once | ok | 1 calls |
| map or coordinates changed at least once | ok | 29 distinct (map,x,y) |
| no single-colour screen for more than 60s | ok | longest 0s |

## Warnings

- 1 single-colour screenshot(s)

## Timeline (one sample per minute)

| t | map | x,y | dialog | battle | party | actions | frames | mono |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 00:00 | 0 Pallet Town | 0,0 | False | False | 0 | 0 | 60 | True |
| 01:00 | 38 Red's House 2F | 7,5 | False | False | 1 | 6604 | 369410 | False |
| 02:00 | 0 Pallet Town | 2,7 | False | False | 1 | 12714 | 751020 | False |
| 03:00 | 0 Pallet Town | 11,3 | False | False | 1 | 19213 | 1123780 | False |
| 04:00 | 0 Pallet Town | 5,2 | False | False | 1 | 26155 | 1495750 | False |
| 05:00 | 12 Route 1 | 11,32 | False | True | 1 | 33723 | 1857870 | False |

## Coordinate trajectory

- 00:00 map 0 Pallet Town (0,0)
- 00:10 map 40 Oak's Lab (2,5)
- 00:20 map 0 Pallet Town (16,7)
- 00:30 map 12 Route 1 (17,31)
- 00:40 map 12 Route 1 (8,29)
- 00:50 map 0 Pallet Town (9,4)
- 01:00 map 38 Red's House 2F (7,5)
- 01:10 map 12 Route 1 (4,30)
- 01:20 map 12 Route 1 (9,28)
- 01:30 map 0 Pallet Town (1,11)
- 01:40 map 0 Pallet Town (18,2)
- 01:50 map 12 Route 1 (7,31)
- 02:00 map 0 Pallet Town (2,7)
- 02:10 map 12 Route 1 (10,32)
- 02:20 map 12 Route 1 (10,35)
- 02:30 map 12 Route 1 (6,28)
- 02:40 map 40 Oak's Lab (1,2)
- 02:50 map 0 Pallet Town (16,6)
- 03:00 map 0 Pallet Town (11,3)
- 03:10 map 0 Pallet Town (16,2)
- 03:20 map 37 Red's House 1F (7,2)
- 03:30 map 0 Pallet Town (14,16)
- 03:40 map 0 Pallet Town (18,10)
- 03:50 map 12 Route 1 (6,28)
- 04:00 map 0 Pallet Town (5,2)
- 04:10 map 12 Route 1 (11,25)
- 04:20 map 37 Red's House 1F (2,4)
- 04:30 map 0 Pallet Town (18,2)
- 04:40 map 0 Pallet Town (1,13)
- 04:50 map 38 Red's House 2F (2,5)
- 05:00 map 12 Route 1 (11,32)

## Driver

- agent rounds: 0, failed: 0, scripted fallback: 0s

## Files

- `samples.jsonl` (every sample), `summary.json`, `server.log`, `driver.log`, `shots/` (every screenshot), `contact-sheet.png` (one per minute)
