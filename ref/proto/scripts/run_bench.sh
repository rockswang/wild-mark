#!/bin/bash
# run_bench.sh: 一键跑原型期 P0/P1/P2 关键基准并汇总（归档脚本，路径已适配仓库结构）。
# 用法: bash ref/proto/scripts/run_bench.sh
set -u
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"; cd "$ROOT"
D=temp; mkdir -p "$D"
TD=testdata/large

echo "=========================================="
echo " 基准开始 $(date '+%F %T')  (CGO-free, go-gui v0.78 + go-edit v0.11)"
echo "=========================================="

build() { printf "build %-24s" "$1:"; CGO_ENABLED=0 go build -o "$2" "./$1" && echo OK; }

echo "[生成测试数据]"
CGO_ENABLED=0 go run ./ref/proto/cmd/gen_testdata "$TD" >/dev/null 2>&1 && echo "  testdata OK"

echo; echo "[P1-a] buffer 加载（默认 Load 受 32MiB 上限；-raw 用 FromBytes 绕过）"
build ref/proto/cmd/p1_load "$D/p1_load.exe"
"$D/p1_load.exe"        "$TD/md_10MiB_norm.md"
"$D/p1_load.exe" -raw   "$TD/md_128MiB_norm.md"

echo; echo "[P1-b] markdown 纯解析"
build ref/proto/cmd/p1_mdparse "$D/p1_mdparse.exe"
"$D/p1_mdparse.exe" "$TD/md_1MiB_norm.md" "$TD/md_32MiB_norm.md"

echo; echo "[P1-c] 预览渲染（mdproto 首帧 + WSS，每个文件约 13s，需真实桌面）"
build ref/proto/cmd/mdproto "$D/mdproto.exe"
bash ref/proto/scripts/measure_mdproto.sh "$TD/md_1MiB_norm.md"  4 1m  2>&1 | grep -E "首次|PEAK|==|gone"
bash ref/proto/scripts/measure_mdproto.sh "$TD/md_10MiB_norm.md" 6 10m 2>&1 | grep -E "首次|PEAK|==|gone"

echo; echo "[P2] 发布体积"
mkdir -p "$D/release"
CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o "$D/release/mdviewer.exe" ./ref/proto/cmd/mdproto
ls -la "$D/release/mdviewer.exe" | awk '{print "  stripped EXE:", $5, "bytes"}'

echo; echo "=========================================="
echo " 完成 $(date '+%T')"
echo "=========================================="
