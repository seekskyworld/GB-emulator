# Playtest T62-rc-gates-7558ce3

**Verdict: PASS** (exit code 0)

## Run
- driver: `agent`
- minutes: `30.0`
- port: `8790`
- url: `http://localhost:8790`
- rom: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/roms/pokered.gb`
- rom_sha1: `ea9bcae617fdf159b045185467ae58b2e4a48b9a`
- gbemu_lib: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/rc/target/release/libgb_ffi.dylib (pinned copy /Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/T62-rc-gates-7558ce3/data/gbemu-lib/libgb_ffi.dylib)`
- gbemu_lib_sha256: `4c1bff22467fd24023c69fc63bd06d92eba9c8951a05c6ee462b02eb9eaa49fb`
- git_head: `7558ce3`
- data_dir: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/reports/playtest/T62-rc-gates-7558ce3/data`
- started: `2026-10-30T01:51:45`
- ended: `2026-10-30T02:21:50`
- server_exit: `None`

## Hard checks

| check | result | detail |
| --- | --- | --- |
| samples taken | ok | 181 samples |
| server process alive throughout | ok |  |
| /health always responds | ok |  |
| /state always 200 with valid fields | ok |  |
| /screenshot always a 160x144 PNG | ok |  |
| frames advance after every /action | ok | 172 ok /action, 0 without frame advance, 0 failed |
| /games/new called exactly once | ok | 1 calls |
| map or coordinates changed at least once | ok | 34 distinct (map,x,y) |
| no single-colour screen for more than 60s | ok | longest 10s (t=0..10s) |

## Warnings

- 5 single-colour screenshot(s)

## Timeline (one sample per minute)

| t | map | x,y | dialog | battle | party | actions | frames | mono |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 00:00 | 0 Pallet Town | 0,0 | False | False | 0 | 0 | 60 | True |
| 01:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 6 | 1082 | False |
| 02:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 12 | 2373 | False |
| 03:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 18 | 4693 | False |
| 04:00 | 38 Red's House 2F | 3,6 | False | False | 0 | 22 | 6004 | False |
| 05:00 | 37 Red's House 1F | 7,1 | False | False | 0 | 29 | 6546 | False |
| 06:00 | 0 Pallet Town | 10,1 | False | False | 0 | 35 | 7086 | False |
| 07:00 | 0 Pallet Town | 9,7 | False | False | 0 | 40 | 8219 | False |
| 08:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 45 | 8810 | False |
| 09:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 48 | 9450 | False |
| 10:00 | 40 Oak's Lab | 5,3 | False | False | 0 | 54 | 10272 | False |
| 11:00 | 40 Oak's Lab | 7,4 | False | False | 0 | 61 | 13312 | False |
| 12:00 | 40 Oak's Lab | 7,4 | False | False | 1 | 66 | 14432 | False |
| 13:00 | 40 Oak's Lab | 6,5 | False | False | 1 | 71 | 15432 | False |
| 14:00 | 40 Oak's Lab | 5,6 | False | False | 1 | 76 | 16312 | False |
| 15:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 81 | 17972 | False |
| 16:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 88 | 19872 | False |
| 17:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 95 | 21752 | False |
| 18:00 | 40 Oak's Lab | 5,6 | False | True | 1 | 101 | 22952 | False |
| 19:00 | 40 Oak's Lab | 5,6 | False | False | 1 | 107 | 24312 | False |
| 20:00 | 40 Oak's Lab | 5,11 | False | False | 1 | 111 | 25192 | False |
| 21:00 | 12 Route 1 | 10,28 | False | False | 1 | 118 | 25752 | False |
| 22:00 | 12 Route 1 | 14,14 | False | True | 1 | 125 | 26383 | False |
| 23:00 | 12 Route 1 | 14,14 | False | True | 1 | 131 | 27365 | False |
| 24:00 | 12 Route 1 | 14,14 | False | False | 1 | 138 | 28776 | False |
| 25:00 | 12 Route 1 | 15,7 | False | True | 1 | 144 | 29576 | False |
| 26:00 | 12 Route 1 | 15,7 | False | True | 1 | 150 | 31056 | False |
| 27:00 | 12 Route 1 | 15,7 | False | True | 1 | 157 | 32307 | False |
| 28:00 | 1 Viridian City | 21,32 | False | False | 1 | 162 | 32987 | False |
| 29:00 | 41 Viridian Pokecenter | 3,3 | False | False | 1 | 167 | 33529 | False |
| 30:00 | 41 Viridian Pokecenter | 3,3 | False | False | 1 | 172 | 34182 | False |

## Coordinate trajectory

- 00:00 map 0 Pallet Town (0,0)
- 01:00 map 38 Red's House 2F (3,6)
- 04:30 map 38 Red's House 2F (5,5)
- 05:00 map 37 Red's House 1F (7,1)
- 05:10 map 37 Red's House 1F (7,5)
- 05:20 map 0 Pallet Town (5,6)
- 05:50 map 0 Pallet Town (10,2)
- 06:00 map 0 Pallet Town (10,1)
- 06:50 map 0 Pallet Town (9,7)
- 07:30 map 0 Pallet Town (12,12)
- 07:40 map 40 Oak's Lab (5,8)
- 07:50 map 40 Oak's Lab (5,3)
- 10:40 map 40 Oak's Lab (5,4)
- 10:50 map 40 Oak's Lab (7,4)
- 12:40 map 40 Oak's Lab (6,5)
- 13:10 map 40 Oak's Lab (5,6)
- 19:40 map 40 Oak's Lab (5,7)
- 20:00 map 40 Oak's Lab (5,11)
- 20:10 map 0 Pallet Town (12,12)
- 20:30 map 0 Pallet Town (9,3)
- 20:40 map 12 Route 1 (10,32)
- 21:00 map 12 Route 1 (10,28)
- 21:10 map 12 Route 1 (8,26)
- 21:20 map 12 Route 1 (12,22)
- 21:30 map 12 Route 1 (14,14)
- 24:50 map 12 Route 1 (15,14)
- 25:00 map 12 Route 1 (15,7)
- 27:20 map 12 Route 1 (15,2)
- 27:30 map 12 Route 1 (11,0)
- 28:00 map 1 Viridian City (21,32)
- 28:10 map 1 Viridian City (21,30)
- 28:30 map 1 Viridian City (19,26)
- 28:40 map 41 Viridian Pokecenter (3,7)
- 28:50 map 41 Viridian Pokecenter (3,4)
- 29:00 map 41 Viridian Pokecenter (3,3)

## Driver

- agent rounds: 22, failed: 0, scripted fallback: 0s
- rounds: `driver-rounds.jsonl`; session ids: `driver-sessions.txt`

## Files

- `samples.jsonl` (every sample), `summary.json`, `server.log`, `driver.log`, `shots/` (every screenshot), `contact-sheet.png` (one per minute)
