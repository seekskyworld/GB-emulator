# Playtest acceptance-20261030-114610

**Verdict: PASS** (exit code 0)

> **STALL WARNING: 2 suspected stall(s), 400s in total** — position unchanged outside battle/dialog for more than 120s while /action kept running. The §4.4 hard checks do not cover this; inspect the screenshots.
>
> - 01:10–05:10 (240s): map 38 Red's House 2F (3,6), 18 /action
> - 11:50–14:30 (160s): map 40 Oak's Lab (7,4), 13 /action

## Run
- driver: `agent`
- minutes: `30.0`
- port: `8780`
- stall_seconds: `120.0`
- fail_on_stall: `False`
- url: `http://localhost:8780`
- rom: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/roms/pokered.gb`
- rom_sha1: `ea9bcae617fdf159b045185467ae58b2e4a48b9a`
- gbemu_lib: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/target/release/libgb_ffi.dylib (pinned copy /Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/acceptance-20261030-114610/data/gbemu-lib/libgb_ffi.dylib)`
- gbemu_lib_sha256: `0d18e627150c31eb9412014bb1901d92d76e6147e57902108e0f90e4de7fc689`
- git_head: `1686902`
- data_dir: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/acceptance-20261030-114610/data`
- started: `2026-10-30T11:46:10`
- ended: `2026-10-30T12:16:14`
- server_exit: `None`

## Hard checks

| check | result | detail |
| --- | --- | --- |
| samples taken | ok | 181 samples |
| server process alive throughout | ok |  |
| /health always responds | ok |  |
| /state always 200 with valid fields | ok |  |
| /screenshot always a 160x144 PNG | ok |  |
| frames advance after every /action | ok | 139 ok /action, 0 without frame advance, 0 failed |
| /games/new called exactly once | ok | 1 calls |
| map or coordinates changed at least once | ok | 25 distinct (map,x,y) |
| no single-colour screen for more than 60s | ok | longest 20s (t=0..20s) |

## Warnings

- STALL suspected: map 38 Red's House 2F (3,6) unchanged for 240s (t=70..310s) outside battle/dialog while 18 /action ran
- STALL suspected: map 40 Oak's Lab (7,4) unchanged for 160s (t=710..870s) outside battle/dialog while 13 /action ran
- coordinates unchanged for 600s (t=880..1480s)
- 6 single-colour screenshot(s)

## Timeline (one sample per minute)

| t | map | x,y | dialog | battle | party | actions | frames | mono |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 00:00 | 0 Pallet Town | 0,0 | False | False | 0 | 0 | 60 | True |
| 01:00 | 0 Pallet Town | 0,0 | False | False | 0 | 5 | 900 | False |
| 02:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 10 | 1820 | False |
| 03:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 15 | 2460 | False |
| 04:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 21 | 2960 | True |
| 05:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 25 | 3800 | False |
| 06:00 | 0 Pallet Town | 5,6 | False | False | 0 | 32 | 4451 | False |
| 07:00 | 0 Pallet Town | 3,6 | False | False | 0 | 37 | 4802 | False |
| 08:00 | 0 Pallet Town | 10,1 | False | False | 0 | 42 | 5342 | False |
| 09:00 | 0 Pallet Town | 10,1 | False | False | 0 | 47 | 5822 | False |
| 10:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 53 | 6633 | False |
| 11:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 58 | 8593 | False |
| 12:00 | 40 Oak's Lab | 7,4 | False | False | 0 | 61 | 9153 | False |
| 13:00 | 40 Oak's Lab | 7,4 | False | False | 1 | 67 | 10313 | False |
| 14:00 | 40 Oak's Lab | 7,4 | False | False | 1 | 68 | 10393 | False |
| 15:00 | 40 Oak's Lab | 5,6 | False | False | 1 | 77 | 11164 | False |
| 16:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 83 | 12004 | False |
| 17:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 89 | 13644 | False |
| 18:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 92 | 14264 | False |
| 19:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 95 | 15104 | False |
| 20:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 99 | 16004 | False |
| 21:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 103 | 16524 | False |
| 22:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 105 | 16964 | False |
| 23:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 110 | 17804 | False |
| 24:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 114 | 18924 | False |
| 25:00 | 0 Pallet Town | 12,12 | False | False | 1 | 121 | 20435 | False |
| 26:00 | 12 Route 1 | 9,28 | False | True | 1 | 127 | 21115 | True |
| 27:00 | 12 Route 1 | 9,28 | False | True | 1 | 132 | 22095 | False |
| 28:00 | 12 Route 1 | 9,28 | False | True | 1 | 132 | 22095 | False |
| 29:00 | 12 Route 1 | 12,25 | False | False | 1 | 139 | 23595 | False |
| 30:00 | 12 Route 1 | 12,25 | False | False | 1 | 139 | 23595 | False |

## Coordinate trajectory

- 00:00 map 0 Pallet Town (0,0)
- 01:10 map 38 Red's House 2F (3,6)
- 05:20 map 38 Red's House 2F (5,6)
- 05:40 map 37 Red's House 1F (7,1)
- 05:50 map 37 Red's House 1F (7,5)
- 06:00 map 0 Pallet Town (5,6)
- 06:10 map 37 Red's House 1F (2,7)
- 06:40 map 0 Pallet Town (5,5)
- 06:50 map 0 Pallet Town (5,6)
- 07:00 map 0 Pallet Town (3,6)
- 07:10 map 0 Pallet Town (2,2)
- 07:20 map 0 Pallet Town (3,2)
- 07:50 map 0 Pallet Town (10,1)
- 09:10 map 0 Pallet Town (10,5)
- 09:20 map 40 Oak's Lab (5,11)
- 09:50 map 40 Oak's Lab (5,5)
- 10:00 map 40 Oak's Lab (5,3)
- 11:50 map 40 Oak's Lab (7,4)
- 14:40 map 40 Oak's Lab (5,6)
- 24:50 map 40 Oak's Lab (5,10)
- 25:00 map 0 Pallet Town (12,12)
- 25:10 map 0 Pallet Town (9,3)
- 25:20 map 12 Route 1 (10,28)
- 25:40 map 12 Route 1 (9,28)
- 28:50 map 12 Route 1 (8,25)
- 29:00 map 12 Route 1 (12,25)

## Driver

- agent rounds: 20, failed: 0, scripted fallback: 0s
- rounds: `driver-rounds.jsonl`; session ids: `driver-sessions.txt`

## Files

- `samples.jsonl` (every sample), `summary.json`, `server.log`, `driver.log`, `shots/` (every screenshot), `contact-sheet.png` (one per minute)
