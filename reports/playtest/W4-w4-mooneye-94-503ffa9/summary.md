# Playtest W4-w4-mooneye-94-503ffa9

**Verdict: PASS** (exit code 0)

## Run
- driver: `agent`
- minutes: `30.0`
- port: `8790`
- url: `http://localhost:8790`
- rom: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/roms/pokered.gb`
- rom_sha1: `ea9bcae617fdf159b045185467ae58b2e4a48b9a`
- gbemu_lib: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/w4final/target/release/libgb_ffi.dylib (pinned copy /Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/W4-w4-mooneye-94-503ffa9/data/gbemu-lib/libgb_ffi.dylib)`
- gbemu_lib_sha256: `59488c5df06a3c6a2963d84f1b2ecc14dba350e8a71eaf698ff21e4b49d74d6f`
- git_head: `503ffa9`
- data_dir: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/W4-w4-mooneye-94-503ffa9/data`
- started: `2026-10-30T02:24:20`
- ended: `2026-10-30T02:54:25`
- server_exit: `None`

## Hard checks

| check | result | detail |
| --- | --- | --- |
| samples taken | ok | 181 samples |
| server process alive throughout | ok |  |
| /health always responds | ok |  |
| /state always 200 with valid fields | ok |  |
| /screenshot always a 160x144 PNG | ok |  |
| frames advance after every /action | ok | 117 ok /action, 0 without frame advance, 0 failed |
| /games/new called exactly once | ok | 1 calls |
| map or coordinates changed at least once | ok | 22 distinct (map,x,y) |
| no single-colour screen for more than 60s | ok | longest 30s (t=280..310s) |

## Warnings

- 11 single-colour screenshot(s)

## Timeline (one sample per minute)

| t | map | x,y | dialog | battle | party | actions | frames | mono |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 00:00 | 0 Pallet Town | 0,0 | False | False | 0 | 0 | 60 | True |
| 01:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 6 | 1160 | False |
| 02:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 9 | 1460 | False |
| 03:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 14 | 2140 | False |
| 04:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 17 | 2280 | True |
| 05:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 20 | 2600 | True |
| 06:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 24 | 2900 | False |
| 07:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 28 | 3060 | False |
| 08:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 34 | 3920 | False |
| 09:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 39 | 4520 | False |
| 10:00 | 37 Red's House 1F | 7,5 | False | False | 0 | 44 | 5071 | False |
| 11:00 | 0 Pallet Town | 1,2 | False | False | 0 | 49 | 5611 | False |
| 12:00 | 0 Pallet Town | 10,1 | False | False | 0 | 52 | 5991 | False |
| 13:00 | 0 Pallet Town | 10,1 | False | False | 0 | 56 | 6611 | False |
| 14:00 | 40 Oak's Lab | 5,11 | False | False | 0 | 59 | 7051 | False |
| 15:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 63 | 7451 | False |
| 16:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 65 | 7811 | False |
| 17:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 69 | 8811 | False |
| 18:00 | 40 Oak's Lab | 7,4 | False | False | 0 | 72 | 9051 | False |
| 19:00 | 40 Oak's Lab | 7,4 | False | False | 0 | 77 | 9451 | False |
| 20:00 | 40 Oak's Lab | 7,4 | False | False | 0 | 79 | 9651 | False |
| 21:00 | 40 Oak's Lab | 7,4 | False | False | 1 | 83 | 10251 | False |
| 22:00 | 40 Oak's Lab | 7,5 | False | False | 1 | 87 | 10931 | False |
| 23:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 95 | 12771 | False |
| 24:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 98 | 13611 | False |
| 25:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 101 | 14851 | False |
| 26:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 101 | 14851 | False |
| 27:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 106 | 15951 | False |
| 28:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 109 | 16371 | False |
| 29:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 112 | 16791 | False |
| 30:00 | 40 Oak's Lab | 4,6 | False | True | 1 | 117 | 17671 | False |

## Coordinate trajectory

- 00:00 map 0 Pallet Town (0,0)
- 01:00 map 38 Red's House 2F (3,6)
- 09:10 map 38 Red's House 2F (4,7)
- 09:20 map 38 Red's House 2F (4,3)
- 09:30 map 37 Red's House 1F (7,1)
- 10:00 map 37 Red's House 1F (7,5)
- 10:10 map 37 Red's House 1F (5,7)
- 10:20 map 0 Pallet Town (5,5)
- 10:30 map 0 Pallet Town (5,6)
- 10:40 map 0 Pallet Town (2,2)
- 11:00 map 0 Pallet Town (1,2)
- 11:10 map 0 Pallet Town (9,2)
- 11:20 map 0 Pallet Town (10,1)
- 13:10 map 0 Pallet Town (11,12)
- 13:40 map 40 Oak's Lab (12,11)
- 14:00 map 40 Oak's Lab (5,11)
- 14:20 map 40 Oak's Lab (5,10)
- 14:30 map 40 Oak's Lab (5,4)
- 14:50 map 40 Oak's Lab (5,3)
- 18:00 map 40 Oak's Lab (7,4)
- 22:00 map 40 Oak's Lab (7,5)
- 22:10 map 40 Oak's Lab (4,6)

## Driver

- agent rounds: 17, failed: 0, scripted fallback: 0s
- rounds: `driver-rounds.jsonl`; session ids: `driver-sessions.txt`

## Files

- `samples.jsonl` (every sample), `summary.json`, `server.log`, `driver.log`, `shots/` (every screenshot), `contact-sheet.png` (one per minute)
