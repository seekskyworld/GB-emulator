#!/usr/bin/env bash
# tools/fetch_refs.sh：并行获取全部参考资料到 $1（默认 refs/），并校验 sha256
set -euo pipefail
DEST="${1:-refs}"; mkdir -p "$DEST"; cd "$DEST"
C="curl -fsSL --retry 4 --retry-all-errors --retry-delay 2 --connect-timeout 15"
gh_tar() { # 仓库 提交 目标目录 [只解出的子路径...]
  local repo=$1 sha=$2 out=$3; shift 3; mkdir -p "$out"
  local name; name="$(basename "$repo")-$sha"
  local paths=(); for p in "$@"; do paths+=("$name/$p"); done
  $C "https://codeload.github.com/$repo/tar.gz/$sha" | tar xz -C "$out" --strip-components=1 "${paths[@]}"
  echo "$sha" > "$out/COMMIT"
}
pids=()
# 最慢的一项（Pan Docs）最先启动
gh_tar gbdev/pandocs 5a8a5ed750424667f6c31b186c2a8b9eb388dce5 pandocs src LICENSE & pids+=($!)
$C -o pandocs-print.html https://gbdev.io/pandocs/print.html & pids+=($!)
gh_tar retrio/gb-test-roms c240dd7d700e5c0b00a7bbba52b53e4ee67b5f15 blargg cpu_instrs instr_timing mem_timing & pids+=($!)
gh_tar mattcurrie/dmg-acid2 8a98ce731f96dde032ffb22ec36dc985d78fdb18 acid2-src & pids+=($!)
( mkdir -p acid2 && $C -o acid2/dmg-acid2.gb https://github.com/mattcurrie/dmg-acid2/releases/download/v1.0/dmg-acid2.gb ) & pids+=($!)
( mkdir -p mooneye && $C https://gekkio.fi/files/mooneye-test-suite/mts-20260714-0944-31510e1/mts-20260714-0944-31510e1.tar.gz -o mooneye/mts.tar.gz && tar xzf mooneye/mts.tar.gz -C mooneye ) & pids+=($!)
gh_tar Gekkio/mooneye-test-suite 31510e12eea6286d36eea060a6adde755e1067aa mooneye-src & pids+=($!)
$C -o Opcodes.json https://raw.githubusercontent.com/gbdev/gb-opcodes/376f61c86fdac2048f7ce5fe838ae756b306017e/Opcodes.json & pids+=($!)
( $C -o gbz80.7 https://raw.githubusercontent.com/gbdev/rgbds/v1.0.4/man/gbz80.7 && (mandoc -T utf8 gbz80.7 2>/dev/null || man ./gbz80.7) | col -b > gbz80.7.txt ) & pids+=($!)
gh_tar NousResearch/pokemon-agent f8a03e4a8bc58f1d9dda6d677a7436cc832a57a9 pokemon-agent & pids+=($!)
gh_tar SingleStepTests/sm83 f9c30210245dd691661db39f5ace022c465ecc2f sst v1 LICENSE & pids+=($!)
( mkdir -p pokered && for f in roms.sha1 Makefile engine/math/random.asm home/vblank.asm; do
    $C -o "pokered/$(basename "$f")" "https://raw.githubusercontent.com/pret/pokered/171e2c204f9c1ff8e577ca387a4bbb009e130843/$f"; done ) & pids+=($!)
fail=0; for p in "${pids[@]}"; do wait "$p" || fail=1; done
[ "$fail" = 0 ] || { echo "ERROR 有下载任务失败，重跑本脚本即可（已完成的会被覆盖）"; exit 1; }
cp acid2-src/img/reference-dmg.png acid2/reference-dmg.png
shasum -a 256 -c - <<'SUMS'
8c5e12f41e0ba5bbca796944f92ffe6de28809198682c4332e38d1b3cf56fcf2  blargg/cpu_instrs/cpu_instrs.gb
fe61349cbaee10cc384b50f356e541c90d1bc380185716706b5d8c465a03cf89  blargg/cpu_instrs/individual/01-special.gb
fb90b0d2b9501910c49709abda1d8e70f757dc12020ebf8409a7779bbfd12229  blargg/cpu_instrs/individual/02-interrupts.gb
ca553e606d9b9c86fbd318f1b916c6f0b9df0cf1774825d4361a3fdff2e5a136  blargg/cpu_instrs/individual/03-op sp,hl.gb
7686aa7a39ef3d2520ec1037371b5f94dc283fbbfd0f5051d1f64d987bdd6671  blargg/cpu_instrs/individual/04-op r,imm.gb
d504adfa0a4c4793436a154f14492f044d38b3c6db9efc44138f3c9ad138b775  blargg/cpu_instrs/individual/05-op rp.gb
17ada54b0b9c1a33cd5429fce5b765e42392189ca36da96312222ffe309e7ed1  blargg/cpu_instrs/individual/06-ld r,r.gb
ab31d3daaaa3a98bdbd9395b64f48c1bdaa889aba5b19dd5aaff4ec2a7d228a3  blargg/cpu_instrs/individual/07-jr,jp,call,ret,rst.gb
974a71fe4c67f70f5cc6e98d4dc8c096057ff8a028b7bfa9f7a4330038cf8b7e  blargg/cpu_instrs/individual/08-misc instrs.gb
b28e1be5cd95f22bd1ecacdd33c6f03e607d68870e31a47b15a0229033d5ba2a  blargg/cpu_instrs/individual/09-op r,r.gb
7f5b8e488c6988b5aaba8c2a74529b7c180c55a58449d5ee89d606a07c53514a  blargg/cpu_instrs/individual/10-bit ops.gb
0ec0cf9fda3f00becaefa476df6fb526c434abd9d4a4beac237c2c2692dac5d3  blargg/cpu_instrs/individual/11-op a,(hl).gb
646067b3d6c79fda810e9c3f1cb7c0efd5abb0a7ac06437c54e65720c15d9925  blargg/instr_timing/instr_timing.gb
791cb418fd054b482a6ada5017e2405b96c2373a0f3914c6b23afeee76fa98ce  blargg/mem_timing/mem_timing.gb
52724532c5709e38e947eb429337c124c38bc68f373874435a7460548098b617  blargg/mem_timing/individual/01-read_timing.gb
eea92d3f4e95aab5910e0f7080916a3c42a2b8deae1ee5d45d1e3751d648f3f6  blargg/mem_timing/individual/02-write_timing.gb
2e9067c670ff8b45916bf321677ad04a6896d06a057dbcb82ae9f208a1ae9c34  blargg/mem_timing/individual/03-modify_timing.gb
464e14b7d42e7feea0b7ede42be7071dc88913f75b9ffa444299424b63d1dff1  acid2/dmg-acid2.gb
ca966d50895c7efef05838590d148c2cbfd7fba57dab986f25b35b4da71abb57  acid2/reference-dmg.png
5f0694e4e9e58441db23ff057a5ab681870d4cfaed91267e99885936db8e6f1d  mooneye/mts.tar.gz
e7e3cd657d8e87b44570474eb3a6ed735501c9a00520f3d6937881203c823bc5  Opcodes.json
5c1e4c3e2e599c5f5f3e8da150f6c132b55dd209ff824b7ae18fd9c749326350  pokered/roms.sha1
fa7f656f78aec939dea1094fd48f555dbde9a796458c5ae2b3e5c236e9720d66  sst/v1/00.json
9833b63c2c9b0ed99adae86c6bde6fd6702ada7a876ff80f2c3adc638ef787c5  sst/v1/cb 00.json
9b9e332ee11019f131e945d40e20ab0c48b52921754dc03fe62cc85b579e6c61  sst/LICENSE
SUMS
echo "OK refs ready: $(pwd)"
