#!/usr/bin/env bash
#
# DeepSeek Harness 启动时的后台更新检查（fire-and-forget，不阻塞启动）。
#
# 每次 app 启动由桌面壳 spawn 一次：查询 GitHub 官方 deepseek-ai/deepseek-harness
# 的最新 tag，与本地已安装 harness 的版本（数据目录里 harness-*/package.json 的
# root version）比较；官方更新时通过 macOS 通知中心提醒一次（同一 tag 只提醒一次，
# 记录在数据目录 update-check.json）。
#
# 仅提醒，不自动下载/覆盖 —— 升级由用户手动决定（见 desktop/README.md 的升级说明）。
#
# 用法: check-update.sh <app-data-dir>
set -uo pipefail

DATA_DIR="${1:-}"
if [ -z "$DATA_DIR" ]; then
  DATA_DIR="$HOME/Library/Application Support/com.deepseek-ai.harness.desktop"
fi

LOG="$DATA_DIR/update-check.log"
STATE="$DATA_DIR/update-check.json"
REPO="deepseek-ai/deepseek-harness"

# 已安装 harness 的 root version（如 0.1.0-rc.5），取最新的 harness-* 目录。
HARNESS_VERSION="unknown"
for d in "$DATA_DIR"/harness-*/; do
  [ -d "$d" ] || continue
  v=$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$d/package.json" 2>/dev/null | head -1)
  if [ -n "$v" ]; then
    HARNESS_VERSION="$v"
    break
  fi
done

# 官方最新 tag（如 dsh-v0.1.3-alpha.2）。
LATEST_TAG=""
LATEST_RAW=$(curl -fsS --max-time 15 "https://api.github.com/repos/$REPO/tags?per_page=1" 2>/dev/null) || LATEST_RAW=""
if [ -n "$LATEST_RAW" ]; then
  LATEST_TAG=$(printf '%s' "$LATEST_RAW" | sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -1)
fi

log() { printf '%s : %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$1" >> "$LOG"; }

if [ -z "$LATEST_TAG" ]; then
  log "check skipped: could not resolve latest tag from GitHub"
  exit 0
fi

# 提取 x.y.z 数字版本段做比较（忽略 rc/alpha 后缀，后缀仅在同号时才有意义）。
num_of() { printf '%s' "$1" | sed -E 's/.*([0-9]+\.[0-9]+\.[0-9]+).*/\1/'; }

# 返回 0 当 $2 的数字版本 > $1。
num_gt() {
  local a b i
  a=$(num_of "$1")
  b=$(num_of "$2")
  if [ -z "$a" ] || [ -z "$b" ]; then return 1; fi
  local ia ib
  IFS='.' read -r -a ia <<<"$a"
  IFS='.' read -r -a ib <<<"$b"
  for i in 0 1 2; do
    local x y
    x=${ia[$i]:-0}; y=${ib[$i]:-0}
    x=${x//[^0-9]/}; y=${y//[^0-9]/}
    if [ "$y" -gt "$x" ]; then return 0; fi
    if [ "$y" -lt "$x" ]; then return 1; fi
  done
  return 1
}

# 上次已提醒过的 tag。
LAST_NOTIFIED=""
if [ -f "$STATE" ]; then
  LAST_NOTIFIED=$(sed -n 's/.*"notified"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$STATE" | head -1)
fi

log "local=$HARNESS_VERSION latest=$LATEST_TAG lastNotified=${LAST_NOTIFIED:-none}"

# 判定是否有可用更新（latest > local）。
HAS_UPDATE="false"
if [ "$HARNESS_VERSION" != "unknown" ] && num_gt "$HARNESS_VERSION" "$LATEST_TAG"; then
  HAS_UPDATE="true"
fi

# 每次检查都写一份结构化结果（当前/最新/是否有更新），供界面层读取展示。
printf '{"current":"%s","latest":"%s","hasUpdate":%s,"checkedAt":"%s"}\n' \
  "$HARNESS_VERSION" "$LATEST_TAG" "$HAS_UPDATE" "$(date '+%Y-%m-%dT%H:%M:%S')" > "$DATA_DIR/update-notice.json"

if [ "$HAS_UPDATE" = "false" ]; then
  log "local is already at/above the latest tag; nothing to show"
  exit 0
fi
if [ "$LATEST_TAG" = "$LAST_NOTIFIED" ]; then
  log "already notified for $LATEST_TAG"
  exit 0
fi

# 弹一个明显的版本提示框，显示当前版本与可升级版本，可一键打开发布页。
# DSH_UPDATE_DIALOG=0 时跳过对话框（用于无人值守测试）。
if [ "${DSH_UPDATE_DIALOG:-1}" = "1" ]; then
  DIALOG_TEXT="检测到 DeepSeek Harness 新版本。

当前版本：${HARNESS_VERSION}
最新版本：${LATEST_TAG}

是否前往 GitHub 查看该版本的发布说明？"
  RESP=$(osascript 2>/dev/null <<APPLESCRIPT
display dialog "$DIALOG_TEXT" buttons {"稍后", "查看发布"} default button "查看发布" with title "DeepSeek Harness 更新可用" with icon caution
APPLESCRIPT
)
  case "$RESP" in
    *"查看发布"*)
      open "https://github.com/$REPO/releases/tag/$LATEST_TAG" >/dev/null 2>&1
      log "opened release page for $LATEST_TAG"
      ;;
    *)
      log "dialog dismissed; no action for $LATEST_TAG"
      ;;
  esac
else
  log "dialog suppressed (DSH_UPDATE_DIALOG=0)"
fi

# 另发一条非模态通知，便于日后在通知中心回溯。
osascript -e "display notification \"官方新版本 ${LATEST_TAG}（当前 ${HARNESS_VERSION}）\" with title \"DeepSeek Harness 更新可用\"" >/dev/null 2>&1 || true

printf '{"notified":"%s","checkedAt":"%s"}\n' "$LATEST_TAG" "$(date '+%Y-%m-%dT%H:%M:%S')" > "$STATE"
log "prompted: $LATEST_TAG (local $HARNESS_VERSION)"

exit 0
