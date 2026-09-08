#!/usr/bin/env bash
#
# prep-bundle.sh — stage the payload for the Tauri DMG build.
#
# Archives the whole harness checkout (with its pnpm node_modules and built
# artifacts — everything needed to run `dsh web` from source) into a tarball
# src-tauri/staging/harness.tar.gz, and downloads the official Node runtime
# (same major used to build the project) into src-tauri/staging/node plus the
# pnpm standalone binary into src-tauri/staging/pnpm (the shell uses pnpm to
# manage plugins in-app).
#
# This is the "fully self-contained" payload: the DMG carries it, and at
# launch the Rust shell extracts the tarball (once) then spawns node from it
# to boot `dsh web`.
#
# Requires: tar, curl, and a prior `pnpm install && pnpm run build` at the
# repository root (so dist/ and lib/ artifacts exist).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC_TAURI="$REPO_ROOT/desktop/src-tauri"
STAGING="$SRC_TAURI/staging"
HARNESS_TARBALL="$STAGING/harness.tar.gz"
NODE_DIR="$STAGING/node"
PNPM_DIR="$STAGING/pnpm"
# Which checkout is archived as the bundled harness. Defaults to the repository
# root; a release can bundle a different upstream checkout (for example an
# official tag the shell has been adapted to) by passing HARNESS_SRC.
HARNESS_SRC="${HARNESS_SRC:-$REPO_ROOT}"

# Keep in sync with the Node used to build the project (engines: ^22.19.0 || >=24.0.0).
NODE_VERSION="${DSH_DESKTOP_NODE_VERSION:-26.7.0}"
ARCH="$(uname -m)"                       # arm64 (Apple Silicon) or x86_64
NODE_ARCH="$([ "$ARCH" = "arm64" ] && echo arm64 || echo x64)"
NODE_TARBALL="/tmp/node-v${NODE_VERSION}-darwin-${NODE_ARCH}.tar.xz"

echo "==> Staging DeepSeek Harness desktop payload"
echo "    Node: v${NODE_VERSION} (darwin-${NODE_ARCH})"

rm -rf "$STAGING"
mkdir -p "$STAGING"

# 1) Archive the harness checkout. Tar stores symlinks as links and does not
#    follow them, so pnpm's node_modules symlink cycles survive the archive
#    and re-extract correctly — unlike a recursive copy.
echo "==> Archiving harness checkout -> $HARNESS_TARBALL"
# Only what the app needs at runtime is archived: source + built lib/dist +
# node_modules. Upstream test suites, docs site, and runnable examples are
# excluded so the packaged harness stays lean. Keep the sync-harness.sh
# exclusion list in step with this one.
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
  -C "$HARNESS_SRC" -czf "$HARNESS_TARBALL" .

# 2) Download + stage the official Node runtime (only bin/node is needed;
#    npm and the rest of the distribution are not required at runtime).
echo "==> Downloading Node runtime"
if [ ! -f "$NODE_TARBALL" ]; then
  curl -fL --http1.1 "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-darwin-${NODE_ARCH}.tar.xz" -o "$NODE_TARBALL"
fi
rm -rf "/tmp/node-v${NODE_VERSION}-darwin-${NODE_ARCH}"
tar -xJf "$NODE_TARBALL" -C /tmp
mkdir -p "$NODE_DIR/bin"
cp "/tmp/node-v${NODE_VERSION}-darwin-${NODE_ARCH}/bin/node" "$NODE_DIR/bin/node"
chmod +x "$NODE_DIR/bin/node"
rm -rf "/tmp/node-v${NODE_VERSION}-darwin-${NODE_ARCH}"

# 3) Download + stage the pnpm standalone binary. The harness `dsh plugin`
#    command shells out to `pnpm` on PATH; the shell puts this bin dir first.
echo "==> Downloading pnpm"
PNPM_VERSION="${DSH_DESKTOP_PNPM_VERSION:-11.7.0}"
mkdir -p "$PNPM_DIR/bin"
if [ ! -f /tmp/pnpm-darwin.tar.gz ]; then
  curl -fL --http1.1 "https://github.com/pnpm/pnpm/releases/download/v${PNPM_VERSION}/pnpm-darwin-${NODE_ARCH}.tar.gz" -o /tmp/pnpm-darwin.tar.gz
fi
rm -rf /tmp/pnpm-darwin-extract
mkdir -p /tmp/pnpm-darwin-extract
tar -xzf /tmp/pnpm-darwin.tar.gz -C /tmp/pnpm-darwin-extract
cp -R /tmp/pnpm-darwin-extract/. "$PNPM_DIR/bin/"
chmod +x "$PNPM_DIR/bin/pnpm"
rm -rf /tmp/pnpm-darwin-extract

# 4) Sanity checks
echo "==> Sanity checks"
"$NODE_DIR/bin/node" --version
"$PNPM_DIR/bin/pnpm" --version
tar -tzf "$HARNESS_TARBALL" | grep -q '^\./apps/cli/src/bin.ts$' && echo "    harness source: ok"
tar -tzf "$HARNESS_TARBALL" | grep -q '^\./apps/web/dist/' && echo "    web dist: ok"
tar -tzf "$HARNESS_TARBALL" | grep -q '^\./node_modules/.pnpm/' && echo "    node_modules: ok"

echo "==> Staged payload size:"
du -sh "$STAGING"
echo "==> Done. Next: cd desktop && npm run dmg"
