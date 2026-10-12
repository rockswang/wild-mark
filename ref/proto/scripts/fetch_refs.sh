#!/bin/bash
# fetch_refs.sh: 解包并重建 ref/ 下的上游参考源码（不入库，需本地生成）。
# 用法: bash ref/proto/scripts/fetch_refs.sh
# 说明: 需要先把模块 zip 放到 ref/dl/（见下），或通过代理下载。
#   go-gui@v0.87.0:  https://proxy.golang.org/github.com/go-gui-org/go-gui/@v/v0.87.0.zip
#   go-gui@v0.78.0:  https://proxy.golang.org/github.com/go-gui-org/go-gui/@v/v0.78.0.zip
#   go-edit@v0.11.0: https://proxy.golang.org/github.com/go-gui-org/go-edit/@v/v0.11.0.zip
# 注意: 解包后 ref/go-gui-v0.78.0 与 ref/go-edit-v0.11.0 需手动应用本地补丁
#       （IME 目标判定、编辑器最大行宽缓存），详见 docs/IME_report.md、docs/P1_report.md。
set -euo pipefail
cd "$(dirname "$0")/../../.."   # 仓库根
DL=ref/dl
mkdir -p "$DL"

unpack() { # $1=zip  $2=模块路径前缀  $3=目标目录
  local zip="$1" prefix="$2" dest="$3"
  [ -f "$zip" ] || { echo "缺失: $zip（请先下载）"; return 1; }
  rm -rf "$dest"; mkdir -p "$dest"
  local tmp="$DL/_x"; rm -rf "$tmp"; mkdir -p "$tmp"
  unzip -q "$zip" -d "$tmp"
  # zip 内顶层为 <prefix>@<version>/，剥掉一层后落到 dest
  local top; top=$(ls "$tmp" | head -1)
  cp -r "$tmp/$top/." "$dest/"
  rm -rf "$tmp"
  echo "解包完成: $zip -> $dest"
}

unpack "$DL/gogui.0.78.0.zip" "github.com/go-gui-org/go-gui"  ref/go-gui-v0.78.0
unpack "$DL/v0.87.0.zip"      "github.com/go-gui-org/go-gui"  ref/go-gui-v0.87.0
unpack "$DL/goedit.zip"       "github.com/go-gui-org/go-edit" ref/go-edit-v0.11.0
echo "提示: 别忘了给 v0.78.0 / v0.11.0 打本地补丁。"
