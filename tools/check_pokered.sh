#!/usr/bin/env bash
# tools/check_pokered.sh：校验 roms/pokered.gb 为美版红 v1.0（缺失或不匹配时非 0 退出）
set -euo pipefail
ROM="${1:-$(cd "$(dirname "$0")/.." && pwd)/roms/pokered.gb}"
EXPECTED=ea9bcae617fdf159b045185467ae58b2e4a48b9a
[ -f "$ROM" ] || { echo "ERROR pokered ROM missing: $ROM"; exit 1; }
actual=$(shasum -a 1 "$ROM" | awk '{print $1}')
[ "$actual" = "$EXPECTED" ] || { echo "ERROR pokered SHA1 mismatch: $actual (expected $EXPECTED)"; exit 1; }
echo "OK pokered SHA1 $actual"
