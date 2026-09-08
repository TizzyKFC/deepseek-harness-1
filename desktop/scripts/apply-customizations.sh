#!/usr/bin/env bash
#
# 把本仓库（官方 deepseek-harness + 桌面定制）的定制，应用到一个新的官方
# harness 源码树，用于"跟随官方升级、保留定制"。
#
# 它只做能安全自动化的部分：
#   A. 复制纯新增目录（desktop/、packages/client/ui-skin-toggle/）
#   B. 校验需要"手工合并"的官方修改文件在新树中存在，并提示
# 真正的逐行合并请按仓库根 CUSTOMIZATIONS.md 的 B 段执行——那是全部需要
# 人工介入的点。合并不了/有冲突时，本脚本不会覆盖任何官方文件。
#
# 用法: apply-customizations.sh <official-harness-tree>
#   例:  desktop/scripts/apply-customizations.sh /tmp/off/deepseek-harness-main
#
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
OFF="${1:-}"
if [ -z "$OFF" ]; then
  echo "用法: apply-customizations.sh <official-harness-tree>" >&2
  exit 1
fi
if [ ! -f "$OFF/packages/bundle/web-app/cordis.patch.yml" ]; then
  echo "错误：不是 deepseek-harness 源码树：$OFF" >&2
  exit 1
fi

echo "==> A. 复制新增定制目录 -> $OFF"
for src in "desktop" "packages/client/ui-skin-toggle"; do
  if [ -e "$REPO/$src" ]; then
    rm -rf "$OFF/$src"
    cp -R "$REPO/$src" "$OFF/$src"
    echo "    copied: $src"
  else
    echo "    WARN 本地缺失 $src（跳过）"
  fi
done

echo ""
echo "==> B. 校验官方新树中需要手工合并的文件"
manual=(
  "packages/bundle/web-app/package.json"
  "packages/bundle/web-app/cordis.patch.yml"
  "packages/bundle/web-app/src/index.ts"
  "packages/host/apiproxy/src/api-proxy.ts"
  "tsconfig.client.json"
)
for f in "${manual[@]}"; do
  if [ -f "$OFF/$f" ]; then
    echo "    [待合并] $f"
  else
    echo "    [路径可能变更] $f （官方新版或已改位置，按 CUSTOMIZATIONS.md 定位）"
  fi
done

echo ""
echo "==> 完成自动部分。请继续："
echo "    1) 打开仓库根 CUSTOMIZATIONS.md，按 B 段把上表 5 个文件逐处合并"
echo "    2) cd '$OFF' && pnpm install && pnpm run build"
echo "    3) 把构建后的新树同步到桌面 app 数据目录（见 desktop/README.md）"
exit 0
