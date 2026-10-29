#!/usr/bin/env bash
# tools/probe_models.sh：并行探测网关可用的完整模型 ID，结果写入 $1（默认 reports/model-probe.md）。不打印密钥。
set -uo pipefail
OUT="${1:-reports/model-probe.md}"; mkdir -p "$(dirname "$OUT")"
: "${ANTHROPIC_BASE_URL:?ANTHROPIC_BASE_URL unset}" "${ANTHROPIC_AUTH_TOKEN:?ANTHROPIC_AUTH_TOKEN unset}"
MODELS="opus-5-5 sonnet-5-5 haiku-5-5 fable-5-1"
tmp=$(mktemp -d)
for m in $MODELS; do
  ( curl -s -m 90 -o "$tmp/$m.json" -w '%{http_code}' "${ANTHROPIC_BASE_URL%/}/v1/messages" \
      -H "x-api-key: $ANTHROPIC_AUTH_TOKEN" -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN" \
      -H "anthropic-version: 2023-06-01" -H "content-type: application/json" \
      -d "{\"model\":\"global.anthropic.claude-$m\",\"max_tokens\":16,\"messages\":[{\"role\":\"user\",\"content\":\"ping\"}]}" \
      > "$tmp/$m.code" ) &
done
wait
{
  echo "# Model probe ($(date '+%Y-%m-%d %H:%M:%S %z'))"; echo
  echo "| requested | HTTP | returned model / error |"; echo "| --- | --- | --- |"
  for m in $MODELS; do
    ret=$(python3 -c 'import json,sys
d=json.load(open(sys.argv[1]))
print(d.get("model") or str(d.get("error",{}).get("message","?"))[:120])' "$tmp/$m.json" 2>/dev/null || echo "?")
    echo "| global.anthropic.claude-$m | $(cat "$tmp/$m.code") | $ret |"
  done
} > "$OUT"
cat "$OUT"; rm -rf "$tmp"
