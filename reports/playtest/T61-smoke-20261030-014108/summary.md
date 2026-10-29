# Playtest 20261030-014108

**Verdict: PASS** (exit code 0)

## Run
- driver: `script`
- minutes: `5.0`
- port: `8780`
- url: `http://localhost:8780`
- rom: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator/roms/pokered.gb`
- rom_sha1: `ea9bcae617fdf159b045185467ae58b2e4a48b9a`
- gbemu_lib: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/play/target/release/libgb_ffi.dylib (pinned copy /Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/play/reports/playtest/20261030-014108/data/gbemu-lib/libgb_ffi.dylib)`
- gbemu_lib_sha256: `9b940298cb478b72e64fa40eb2e97d8d5fdd75df9ffc2cfe69fc622ef08b72e4`
- git_head: `d6639cf+dirty`
- data_dir: `/Users/skyseek/Desktop/project/open/research-platform/GB-emulator-lanes/play/reports/playtest/20261030-014108/data`
- started: `2026-10-30T01:41:08`
- ended: `2026-10-30T01:46:11`
- server_exit: `None`

## Hard checks

| check | result | detail |
| --- | --- | --- |
| samples taken | ok | 31 samples |
| server process alive throughout | ok |  |
| /health always responds | ok |  |
| /state always 200 with valid fields | ok |  |
| /screenshot always a 160x144 PNG | ok |  |
| frames advance after every /action | ok | 40661 ok /action, 0 without frame advance, 0 failed |
| /games/new called exactly once | ok | 1 calls |
| map or coordinates changed at least once | ok | 3 distinct (map,x,y) |
| no single-colour screen for more than 60s | ok | longest 0s |

## Warnings

- 1 single-colour screenshot(s)

## Timeline (one sample per minute)

| t | map | x,y | dialog | battle | party | actions | frames | mono |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 00:00 | 0 Pallet Town | 0,0 | False | False | 0 | 0 | 60 | True |
| 01:00 | 12 Route 1 | 10,34 | False | True | 1 | 8153 | 408320 | False |
| 02:00 | 12 Route 1 | 10,34 | False | True | 1 | 16223 | 823580 | False |
| 03:00 | 12 Route 1 | 10,34 | False | True | 1 | 24448 | 1247288 | False |
| 04:00 | 12 Route 1 | 10,34 | False | True | 1 | 32522 | 1663078 | False |
| 05:00 | 12 Route 1 | 10,34 | False | True | 1 | 40660 | 2082738 | False |

## Coordinate trajectory

- 00:00 map 0 Pallet Town (0,0)
- 00:10 map 40 Oak's Lab (6,2)
- 00:20 map 12 Route 1 (10,34)

## Driver

- agent rounds: 0, failed: 0, scripted fallback: 0s

## Files

- `samples.jsonl` (every sample), `summary.json`, `server.log`, `driver.log`, `shots/` (every screenshot), `contact-sheet.png` (one per minute)
