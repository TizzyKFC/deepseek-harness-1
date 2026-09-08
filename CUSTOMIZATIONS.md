# DeepSeek Harness 本地定制清单

本仓库 = 官方 `deepseek-ai/deepseek-harness` 源码 + 桌面化定制。

## 现状（2026-09）：两条线

- **Shipped（DMG 内置）基线 = 官方 `0.1.3-alpha.2`（直接适配，已端到端验证）**。打 DMG 时 `prep-bundle.sh` 用 `HARNESS_SRC` 指向官方 0.1.3 checkout，harness 代码**不并入**本仓库 `packages/` 源码树。
- **本仓库 `packages/` 源码树 = 0.1.0-rc.5 定制老树**（历史线）。下文 A/B 段 + `packages/client/ui-skin-toggle/` 皮肤切换都是在这一棵老树上做的；皮肤要在 0.1.3 上按新 settings API 重写，**延后**，故 0.1.3 基线不含皮肤。

## 0.1.3 直接适配（改动全部在 desktop/，提交 1bfba81 + prep 参数化）

1. `desktop/src-tauri/src/lib.rs`
   - spawn 参数加 `--no-open`：0.1.3 CLI 才支持；不传则浏览器接管打印第二行 readiness 到已关闭的 stdout → node `EPIPE` 崩溃（0.1.0 CLI 不认该参数）。
   - 认证握手 `auth_handshake_js`：0.1.3 用 `?token=` 换 HttpOnly cookie（GET `/?token=` 命中 → 303 `Location:/` + `Set-Cookie dsh-auth-…`，干净 `/` 带 cookie 才返回 index）。WKWebView **顶层导航的 303 Set-Cookie 不入 cookie jar** → 窗口停在 401『dsh web authentication required』。修复：在 401 页上执行同源 `fetch('/?token=…',{credentials:'include',redirect:'follow'})`（子资源路径正确存 cookie）再 `location.href='/'`；导航后 1.2/2.6/4.2s 用 `run_on_main_thread`+`eval` 重试三次（闭包内 handle 需预克隆，否则 E0505）。
2. `desktop/scripts/prep-bundle.sh`：新增 `HARNESS_SRC` 环境变量（默认仓库根），允许把别的 checkout 打成 DMG 内置 harness 基线。

## A. 纯新增（官方上游没有，直接整树复制即可）【0.1.0 老树定制】

| 路径 | 内容 |
|---|---|
| `desktop/` | 整个 Tauri 桌面壳：Rust（`src-tauri/`，含启动更新检查、插件管理器、主题切换命令）、`scripts/prep-bundle.sh`、`scripts/*`、`README.md`。官方打包时会 `--exclude desktop`，不进 harness tar，但代码随本仓库管理 |
| `packages/client/ui-skin-toggle/` | 新 client 插件 `@deepseek-ai/dsh-client-ui-skin-toggle`：WebUI 侧边栏"主题：Blue Fantasy/默认"切换按钮 + vendored Blue Fantasy 皮肤资产（CSS/鲸鱼/图标）。依赖 `dsh-client-runtime`、`dsh-client-ui-settings`（settingsScope）、`@deepseek-ai/cordis` |

复制新增部分：
```sh
cp -R desktop/            <官方新树>/desktop/
cp -R packages/client/ui-skin-toggle/ <官方新树>/packages/client/ui-skin-toggle/
```

## B. 修改官方文件（6 处，官方升级时需逐处合并）【0.1.0 老树定制】

### 1. `packages/bundle/web-app/package.json`
- `dependencies` 加：
  - `"@deepseek-ai/dsh-settings": "workspace:^"`
  - `"@deepseek-ai/dsh-client-ui-skin-toggle": "workspace:^"`
- `dependencies` 删：`"@linxin666/dsh-client-ui-web-ui-settings": "^0.1.9"`（空面板容器，已弃用）

### 2. `packages/bundle/web-app/cordis.patch.yml`
- 删掉 `ui-web-ui-settings` 整行及其说明注释（原"Web UI settings group container (hosts family cards)"）。
- 在 `ui-task-board` 之后、`locale` 之前保持有：
  ```yaml
  # Blue Fantasy skin toggle: a sidebar row switching the built-in skin and
  # the default theme via a durable setting (hot-reloaded, no restart). Sits
  # after ui-settings so its settingsScope inject resolves.
  - id: ui-skin-toggle
    name: '@deepseek-ai/dsh-client-ui-skin-toggle'
  ```
  （前提：`ui-settings` 的 patch 行在本插件**之前**。）

### 3. `packages/bundle/web-app/src/index.ts`
- 顶部加（拉取 settings Context merge + branded namespace 构造器）：
  ```ts
  import type {} from '@deepseek-ai/dsh-settings'
  import { settingsNamespace } from '@deepseek-ai/dsh-settings'
  ```
- `apply()` 内开头加（注册 durable skin namespace，供浏览器端皮肤切换热载/持久化）：
  ```ts
  ctx.inject(['settings'], (settingsCtx) => {
    settingsCtx.settings.register(settingsNamespace('skin'), z.object({
      theme: z.string().default('blue-fantasy'),
    }), { applies: 'live' })
  })
  ```
  （需确认 `z` 已从 `@deepseek-ai/schemastery` import。）

### 4. `packages/host/apiproxy/src/api-proxy.ts`
- `WEB_SETTINGS_NAMESPACES` 数组加 `'skin'`（否则 settings RPC 返回 `settings-not-exposed`，浏览器写不进去）。加注释说明属 `ui-skin-toggle` 主题设置。

### 5. `tsconfig.client.json`（仓库根）
- `references` 数组加：`{ "path": "./packages/client/ui-skin-toggle" }`（否则 `tsc -b` 不构建新包，tsdown 报 UNRESOLVED_ENTRY）。

### 6. 若官方 client 聚合需要
- 若新版把 client 包聚合进某 manifest/`tsdown`/bundle 清单，把 `ui-skin-toggle` 加入与 `ui-task-board` 同级的位置（skin-toggle 自己带 `cordis.patch.yml` 里注册的 client bundle 入口，见 `packages/client/ui-skin-toggle/tsdown.config.ts`）。

## 打 DMG / 升级流程（0.1.3 直接适配）

1. 取官方 0.1.3 checkout（本会话：`/tmp/upgrade-test/deepseek-harness-dsh-v0.1.3-alpha.2`，已 `pnpm install --force && pnpm run build`）。
2. 打 staging：`cd desktop && HARNESS_SRC=<官方树> bash scripts/prep-bundle.sh`（压缩约 5 分钟，产物 staging/harness.tar.gz ≈2.6G；staging 已就绪可跳过）。
3. 建 DMG：`cd desktop && npx tauri build --bundles dmg`（产物 `src-tauri/target/release/bundle/dmg/DeepSeek Harness_<ver>_aarch64.dmg`）。
4. 验证（真机全链路）：装载 DMG → `ditto` 覆盖 `/Applications/DeepSeek Harness.app` → **删** `data_dir/harness-<ver>`（强制从新包 tar 重解压，`dsh/` 用户数据不动）→ 启动：解压出的 harness `package.json` 版本应为 `0.1.3-alpha.2`，日志见 `navigate() ok` + 三次 `auth handshake eval ok`，node 与 WebKit 建两条 ESTABLISHED 连接（UI 已加载），窗口截图可见 0.1.3 UI。

把 0.1.3 checkout 替换成新官方版本即同流程升级；壳（desktop/）改完只重编译不重打 harness。

> A/B 段与 `apply-customizations.sh` 描述的是 0.1.0 老树定制，皮肤在 0.1.3 落地前仅供历史参考。
