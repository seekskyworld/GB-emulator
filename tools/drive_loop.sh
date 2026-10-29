#!/usr/bin/env bash
# tools/drive_loop.sh — 实玩驾驶外层循环（PLAN §4.4 --driver agent）。
#
# 每一轮开一个全新的 `claude -p` 会话（Sonnet 5.5 high），只允许 `Bash(curl *localhost*)` 与 Read；
# 会话读 /objectives、/state、/map/ascii、截图，执行 5–10 个 /action，写回 /objectives 后退出。
# 循环直到时间用完。由 tools/playtest.py --driver agent 启动，也可以单独运行（服务器须已在运行）。
#
# 环境变量（playtest.py 会设置）：
#   PORT           服务器端口（默认 8765）；URL 默认 http://localhost:$PORT
#   RUN_DIR        报告目录（必填）：写 driver-sessions.txt（每轮开始时一行 started，结束时一行结果）、
#                  driver-rounds.jsonl、driver-rounds/
#   SECONDS_TOTAL  总驾驶秒数（默认 MINUTES*60，MINUTES 默认 30）
#   MODEL          默认 global.anthropic.claude-sonnet-5-5；EFFORT 默认 high
#   ROUND_TIMEOUT  单轮上限秒数（默认 180）；超过即杀掉并记为失败
#   FALLBACK_SECONDS  连续 3 轮失败后 scripted 兜底的秒数（默认 60）
#   PYTHON         运行 scripted_driver.py 的解释器（默认 python3）
#   CLAUDE_BIN     claude 可执行文件（默认 claude；离线测试时替换为假的）
#   GBEMU_REFS     资料目录（默认 <repo>/refs），用于在 prompt 中引用 pokemon-agent 的 SKILL.md
#
# 失败处理：非 0 退出、超过 ROUND_TIMEOUT、或本轮没有执行任何 /action，记为失败轮次并立即开下一轮；
# 连续 3 轮失败时用 scripted_driver.py 驾驶 FALLBACK_SECONDS 秒，记入 driver-rounds.jsonl（kind=scripted）。
# playtest.py 汇总 scripted 累计时长，超过 3 分钟则本次实玩无效。

set -u

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"

PORT="${PORT:-8765}"
URL="${URL:-http://localhost:$PORT}"
RUN_DIR="${RUN_DIR:?RUN_DIR must be set (report directory)}"
MINUTES="${MINUTES:-30}"
SECONDS_TOTAL="${SECONDS_TOTAL:-$(awk "BEGIN{print int($MINUTES*60)}")}"
MODEL="${MODEL:-global.anthropic.claude-sonnet-5-5}"
EFFORT="${EFFORT:-high}"
ROUND_TIMEOUT="${ROUND_TIMEOUT:-180}"
FALLBACK_SECONDS="${FALLBACK_SECONDS:-60}"
PYTHON="${PYTHON:-python3}"
CLAUDE_BIN="${CLAUDE_BIN:-claude}"
REFS="${GBEMU_REFS:-$REPO/refs}"
SKILL="$REFS/pokemon-agent/skill/SKILL.md"
PROMPT_TEMPLATE="$HERE/driver_prompt.md"

mkdir -p "$RUN_DIR/driver-rounds" "$RUN_DIR/driver-shots"
SESSIONS="$RUN_DIR/driver-sessions.txt"
ROUNDS="$RUN_DIR/driver-rounds.jsonl"
[ -f "$SESSIONS" ] || printf 'round\tsession_id\trc\tseconds\tactions\tstatus\n' > "$SESSIONS"

now() { date +%s; }
END=$(( $(now) + SECONDS_TOTAL ))
CHILD=""

CUR_ROUND=""   # 进行中的 agent 轮次（被时间到中断时也要把会话 ID 记下来，R4）
CUR_SID=""
CUR_START=0
CUR_BEFORE=-1

cleanup() {
  if [ -n "$CHILD" ] && kill -0 "$CHILD" 2>/dev/null; then
    kill -TERM "$CHILD" 2>/dev/null
    sleep 2
    kill -KILL "$CHILD" 2>/dev/null
  fi
  if [ -n "$CUR_ROUND" ]; then
    local dur=$(( $(now) - CUR_START )) after actions=-1
    after="$(actions_total)"
    if [ "$CUR_BEFORE" -ge 0 ] && [ "$after" -ge 0 ]; then actions=$(( after - CUR_BEFORE )); fi
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$CUR_ROUND" "$CUR_SID" "-" "$dur" "$actions" interrupted >> "$SESSIONS"
    json_record kind=agent round="$CUR_ROUND" session_id="$CUR_SID" rc=-1 timeout=false seconds="$dur" \
        actions="$actions" ok=true status=interrupted >> "$ROUNDS"
    echo "[drive_loop] round $CUR_ROUND interrupted at end of run (session $CUR_SID)"
  fi
  echo "[drive_loop] stopped"
  exit 0
}
trap cleanup INT TERM

# 服务端 /action 计数（来自 gbemu.launch 的 /gbemu/stats）；取不到时输出 -1。
actions_total() {
  local js
  js="$(curl -s -m 5 "$URL/gbemu/stats" 2>/dev/null)" || { echo -1; return; }
  printf '%s' "$js" | sed -n 's/.*"actions_total":[ ]*\([0-9][0-9]*\).*/\1/p' | head -n1 | grep . || echo -1
}

# 把 driver_prompt.md 中的占位符替换成本轮的值。
render_prompt() {
  local round="$1" remaining="$2" out="$3"
  sed -e "s|{{PORT}}|$PORT|g" \
      -e "s|{{ROUND}}|$round|g" \
      -e "s|{{REMAINING}}|$remaining|g" \
      -e "s|{{SHOT}}|$RUN_DIR/driver-shots/screen.png|g" \
      -e "s|{{SKILL}}|$SKILL|g" \
      "$PROMPT_TEMPLATE" > "$out"
}

new_uuid() {
  if command -v uuidgen >/dev/null 2>&1; then uuidgen | tr 'A-Z' 'a-z'
  else "$PYTHON" -c 'import uuid; print(uuid.uuid4())'; fi
}

# json 输出里的 session_id（取不到时为空）。
json_session_id() {
  sed -n 's/.*"session_id":[ ]*"\([^"]*\)".*/\1/p' "$1" 2>/dev/null | head -n1
}

json_record() {  # key=value ... → 一行 JSON（值为数字/true/false 时不加引号）
  local first=1 kv k v
  printf '{'
  for kv in "$@"; do
    k="${kv%%=*}"; v="${kv#*=}"
    [ $first -eq 1 ] || printf ','
    first=0
    if [[ "$v" =~ ^-?[0-9]+$ || "$v" == true || "$v" == false ]]; then printf '"%s":%s' "$k" "$v"
    else v="${v//\\/\\\\}"; v="${v//\"/\\\"}"; printf '"%s":"%s"' "$k" "$v"; fi
  done
  printf '}\n'
}

run_fallback() {
  local start end rc
  echo "[drive_loop] 3 failed rounds in a row: scripted driver for ${FALLBACK_SECONDS}s"
  start=$(now)
  "$PYTHON" "$HERE/scripted_driver.py" --url "$URL" --seconds "$FALLBACK_SECONDS" --intro auto \
      --log "$RUN_DIR/driver-actions.jsonl" &
  CHILD=$!
  wait "$CHILD"; rc=$?
  CHILD=""
  end=$(now)
  json_record kind=scripted round="$ROUND" rc="$rc" seconds=$(( end - start )) ok=true >> "$ROUNDS"
}

ROUND=0
FAILS=0
echo "[drive_loop] url=$URL seconds=$SECONDS_TOTAL model=$MODEL effort=$EFFORT timeout=${ROUND_TIMEOUT}s"
while [ "$(now)" -lt "$END" ]; do
  ROUND=$(( ROUND + 1 ))
  remaining=$(( END - $(now) ))
  tag=$(printf 'round-%03d' "$ROUND")
  prompt="$RUN_DIR/driver-rounds/$tag.prompt.md"
  out="$RUN_DIR/driver-rounds/$tag.json"
  err="$RUN_DIR/driver-rounds/$tag.stderr"
  render_prompt "$ROUND" "$remaining" "$prompt"
  sid="$(new_uuid)"
  before="$(actions_total)"
  start=$(now)

  # prompt 走 stdin：--allowedTools 是可变参数，放在它后面的位置参数会被当成工具名。
  CUR_ROUND="$ROUND"; CUR_SID="$sid"; CUR_START="$start"; CUR_BEFORE="$before"
  # 先记下会话 ID：即使本脚本随后被直接杀掉（trap 来不及跑），导出会话（R4）也不会漏掉这一轮。
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$ROUND" "$sid" "-" 0 0 started >> "$SESSIONS"
  "$CLAUDE_BIN" -p --model "$MODEL" --effort "$EFFORT" --output-format json \
      --session-id "$sid" --permission-mode dontAsk --tools "Bash,Read" \
      --allowedTools "Bash(curl *localhost*)" "Read" < "$prompt" > "$out" 2> "$err" &
  CHILD=$!
  timed_out=false
  while kill -0 "$CHILD" 2>/dev/null; do
    if [ $(( $(now) - start )) -ge "$ROUND_TIMEOUT" ]; then
      timed_out=true
      kill -TERM "$CHILD" 2>/dev/null; sleep 3; kill -KILL "$CHILD" 2>/dev/null
      break
    fi
    sleep 1 & wait $!   # 用 wait 等待，信号到达时 trap 立即执行
  done
  wait "$CHILD"; rc=$?
  CHILD=""
  CUR_ROUND=""
  dur=$(( $(now) - start ))
  after="$(actions_total)"
  if [ "$before" -ge 0 ] && [ "$after" -ge 0 ]; then actions=$(( after - before )); else actions=-1; fi
  jsid="$(json_session_id "$out")"
  [ -n "$jsid" ] && sid="$jsid"

  ok=true; status=ok
  if [ "$timed_out" = true ]; then ok=false; status=timeout
  elif [ "$rc" -ne 0 ]; then ok=false; status="exit-$rc"
  elif [ "$actions" -eq 0 ]; then ok=false; status=no-actions
  fi
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$ROUND" "$sid" "$rc" "$dur" "$actions" "$status" >> "$SESSIONS"
  json_record kind=agent round="$ROUND" session_id="$sid" rc="$rc" timeout="$timed_out" seconds="$dur" \
      actions="$actions" ok="$ok" status="$status" >> "$ROUNDS"
  echo "[drive_loop] $tag session=$sid rc=$rc ${dur}s actions=$actions status=$status"

  if [ "$ok" = true ]; then
    FAILS=0
  else
    FAILS=$(( FAILS + 1 ))
    if [ "$FAILS" -ge 3 ] && [ "$(now)" -lt "$END" ]; then
      run_fallback
      FAILS=0
    fi
  fi
done
echo "[drive_loop] done after $ROUND rounds"
