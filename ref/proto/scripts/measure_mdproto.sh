#!/bin/bash
# measure_mdproto.sh: 对给定文件运行 mdproto，采样首帧耗时与峰值 WSS（需真实桌面会话）。
# 用法: bash ref/proto/scripts/measure_mdproto.sh <file> <duration_s> [tag]
F=${1:?need file}
DUR=${2:-12}
TAG=${3:-normal}
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"; cd "$ROOT"
D="$ROOT/temp"; mkdir -p "$D"
[ -x "$D/mdproto.exe" ] || CGO_ENABLED=0 go build -o "$D/mdproto.exe" ./ref/proto/cmd/mdproto

taskkill //f //im mdproto.exe 2>/dev/null | head -1
sleep 0.5
"$D/mdproto.exe" "$F" > "$D/md_$TAG.log" 2>&1 &
sleep 2
PID=$(tasklist //fo csv //nh 2>/dev/null | grep -i mdproto | head -1 | sed 's/","/ /g' | awk '{print $2}')
echo "== $TAG file=$F PID=$PID =="
PEAK=0; PEAKT=0
for i in $(seq 1 "$DUR"); do
  sleep 1
  WSS=$(tasklist //fo csv //nh //fi "PID eq $PID" 2>/dev/null | grep -i mdproto | head -1 | sed 's/","/ /g' | awk '{print $5}')
  if [ -n "$WSS" ]; then
    WSS_KB=$(echo "$WSS" | tr -d ' K')
    echo "  t=${i}s WSS=${WSS_KB}KB"
    if [ "$WSS_KB" -gt "$PEAK" ]; then PEAK=$WSS_KB; PEAKT=$i; fi
  else
    echo "  t=${i}s process gone"; break
  fi
done
echo "  PEAK WSS = $((PEAK/1024)) MB @t=${PEAKT}s"
echo "== log =="; cat "$D/md_$TAG.log" 2>/dev/null
taskkill //f //im mdproto.exe 2>/dev/null | head -1
