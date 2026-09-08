#!/usr/bin/env bash
#
# 升级桌面 app 内已解压的 harness 到「当前仓库源码」——不重打包 DMG。
#
# 适用：收到 app 启动更新提醒、或自己改了 harness 源码后，想让 app 用上新代码。
# 仓库文件保持完整（含官方测试，便于跟随上游），本脚本只把「运行所需」归档进
# app 数据目录：源码 + 构建产物 + node_modules，排除官方测试/文档/示例。
#
# 用法: sync-harness.sh [<app-data-dir>]
#   app-data-dir 默认 ~/Library/Application Support/com.deepseek-ai.harness.desktop
#
# 前置: 仓库根已 `pnpm install && pnpm run build`（lib/dist 产物会打进归档）。
set -euo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
DATA_DIR="${1:-$HOME/Library/Application Support/com.deepseek-ai.harness.desktop}"

# app 版本（桌面三处版本同步之一 Cargo.toml）决定 harness-<版本> 目录名。
APP_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$REPO/desktop/src-tauri/Cargo.toml" | head -1)"
if [ -z "$APP_VERSION" ]; then
  echo "错误：无法从 desktop/src-tauri/Cargo.toml 读取 app 版本" >&2
  exit 1
fi
HARNESS="$DATA_DIR/harness-$APP_VERSION"

echo "==> 目标 harness 目录：$HARNESS"

# 0) 退出正在运行的 app（覆盖旧 harness 前必须）。
if pgrep -f 'DeepSeek Harness.app/Contents/MacOS/dsh-desktop' >/dev/null 2>&1; then
  echo "==> 检测到 app 运行中，正在退出…"
  pkill -f 'DeepSeek Harness.app/Contents/MacOS/dsh-desktop' || true
  pkill -f 'staging/node/bin/node' || true
  sleep 2
fi

# 1) 归档（排除与 prep-bundle.sh 保持一致：官方测试/文档/示例）。
TARBALL="/tmp/harness-sync.tar.gz"
echo "==> 归档（排除官方测试/文档/示例）…"
tar \
  --exclude='./.git' \
  --exclude='./desktop' \
  --exclude='./docs' \
  --exclude='./website' \
  --exclude='./examples' \
  --exclude='*/tests/*' \
  --exclude='*/__tests__/*' \
  --exclude='*/__snapshots__/*' \
  --exclude='*.spec.ts' \
  --exclude='*.spec.tsx' \
  --exclude='*.test.ts' \
  --exclude='*.test.tsx' \
  --exclude='*.e2e.ts' \
  --exclude='*.snap' \
  --exclude='*.tsbuildinfo' \
  -C "$REPO" -czf "$TARBALL" .

# 2) 重建 harness-<版本> 目录并解压（tar 保留 pnpm 符号链接环）。
echo "==> 重建并解压到 $HARNESS"
rm -rf "$HARNESS"
mkdir -p "$HARNESS"
tar -xzf "$TARBALL" -C "$HARNESS"
echo ok > "$HARNESS/.extracted"
rm -f "$TARBALL"

echo "==> 完成。请重新打开 DeepSeek Harness。"
echo "    用户数据（dsh/ 会话/设置/插件）不受影响；更新检查将按新版本判定。"
exit 0
