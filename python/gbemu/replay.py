"""Replay a scripted/agent action log directly on the backend (no server).

    GBEMU_LIB=… python -m gbemu.replay --rom roms/pokered.gb \\
        reports/playtest/<run>/driver-actions.jsonl --upto 4078 --png /tmp/at.png --watch 600

The emulator is deterministic, so replaying the ``actions`` of every logged
``/action`` request (in order, from a fresh boot plus the 60 frames that
``/games/new`` ticks) reproduces the run frame for frame.  The action
semantics mirror pokemon-agent's ``server._execute_action``:
``press_X``/``walk_X`` = hold 8 + wait 12 frames, ``hold_X_N``, ``wait_N``.
``a_until_dialog_end`` depends on the server's state reader and is not supported.
"""

from __future__ import annotations

import argparse
import collections
import json
import sys

from gbemu.backend import GbEmuEmulator

# pokemon_agent.memory.red addresses
MAP_ID, MAP_Y, MAP_X = 0xD35E, 0xD361, 0xD362


def apply_action(emu: GbEmuEmulator, action: str) -> None:
    parts = action.strip().lower().split("_")
    if parts[0] in ("press", "walk") and len(parts) >= 2:
        emu.press("_".join(parts[1:]), 8)
        emu.tick(12)
    elif parts[0] == "hold" and len(parts) >= 3:
        emu.press("_".join(parts[1:-1]), int(parts[-1]))
    elif parts[0] == "wait" and len(parts) == 2:
        emu.tick(int(parts[1]))
    else:
        raise ValueError(f"cannot replay action {action!r}")


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("log", help="driver-actions.jsonl (one /action request per line)")
    ap.add_argument("--rom", required=True)
    ap.add_argument("--upto", type=int, default=None, help="replay only the first N requests")
    ap.add_argument("--boot-frames", type=int, default=60, help="frames before the first request (/games/new ticks 60)")
    ap.add_argument("--png", default=None, help="save the screen after the replay")
    ap.add_argument("--watch", type=int, default=0, help="then run N more frames and report PC/WRAM activity")
    args = ap.parse_args(argv)

    reqs = [json.loads(line) for line in open(args.log, encoding="utf-8") if line.strip()]
    if args.upto is not None:
        reqs = reqs[: args.upto]
    emu = GbEmuEmulator()
    emu.load(args.rom)
    emu.tick(args.boot_frames)
    mismatches = 0
    for i, r in enumerate(reqs):
        for a in r.get("actions", []):
            apply_action(emu, a)
        got = (emu.read_u8(MAP_ID), emu.read_u8(MAP_X), emu.read_u8(MAP_Y))
        want = (r.get("map"), r.get("x"), r.get("y"))
        if None not in want and got != want:
            mismatches += 1
            if mismatches <= 5:
                print(f"request {i}: replay (map,x,y)={got} but log says {want}")
    print(f"replayed {len(reqs)} requests; frame {emu.frame_count}; (map,x,y)="
          f"{(emu.read_u8(MAP_ID), emu.read_u8(MAP_X), emu.read_u8(MAP_Y))}; mismatches {mismatches}")
    print("regs", {k: (hex(v) if isinstance(v, int) and not isinstance(v, bool) else v)
                   for k, v in emu.regs().items()})
    if args.png:
        emu.get_screen().save(args.png)
        print("screen ->", args.png)
    if args.watch:
        pcs = collections.Counter()
        before = emu.read_range(0xC000, 0x2000)
        for _ in range(args.watch):
            emu.tick(1)
            pcs[hex(emu.regs()["pc"])] += 1
        after = emu.read_range(0xC000, 0x2000)
        changed = [hex(0xC000 + i) for i in range(0x2000) if before[i] != after[i]]
        print(f"PC at frame ends over {args.watch} frames:", pcs.most_common(8))
        print(f"WRAM bytes changed: {len(changed)}", changed[:40])
    emu.close()
    return 0 if mismatches == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
