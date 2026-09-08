# DeepSeek Harness 本地定制清单

本仓库 = 官方 `deepseek-ai/deepseek-harness` 源码 + 桌面化定制。本文件用于在**跟随官方升级**时精确重放定制。升级流程见文末。

> 现状：本地官方部分 ≈ 官方 `0.1.0` 系列（package.json `0.1.0-rc.5`），不是某个 tag 的精确快照。官方已到 `dsh-v0.1.3-alpha.2`。

## A. 纯新增（官方上游没有，直接整树复制即可）

| 路径 | 内容 |
|---|---|
| `desktop/` | 整个 Tauri 桌面壳：Rust（`src-tauri/`，含启动更新检查、插件管理器、主题切换命令）、`scripts/prep-bundle.sh`、`scripts/*`、`README.md`。官方打包时会 `--exclude desktop`，不进 harness tar，但代码随本仓库管理 |
| `packages/client/ui-skin-toggle/` | 新 client 插件 `@deepseek-ai/dsh-client-ui-skin-toggle`：WebUI 侧边栏"主题：Blue Fantasy/默认"切换按钮 + vendored Blue Fantasy 皮肤资产（CSS/鲸鱼/图标）。依赖 `dsh-client-runtime`、`dsh-client-ui-settings`（settingsScope）、`@deepseek-ai/cordis` |

复制新增部分：
```sh
cp -R desktop/            <官方新树>/desktop/
cp -R packages/client/ui-skin-toggle/ <官方新树>/packages/client/ui-skin-toggle/
```

## B. 修改官方文件（6 处，官方升级时需逐处合并）

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

## 升级流程（跟随官方）

1. 获取官方新树（推荐用 git：本仓库已配 `upstream` 指向官方，`git fetch upstream` 后取新 tag/分支内容到临时目录；或下载官方 tag tar）。
2. 复制 A 段两个新增目录进新树。
3. 按 B 段逐处合并 6 处修改（官方若改动了同一文件，冲突点就在这几处，手工对齐即可——这是**全部**需要人工的地方）。
4. `pnpm install && pnpm run build`（新树内）。
5. 把新树同步进桌面 app：见 `desktop/README.md` / 会话记忆的 tar 同步方案（`data_dir/harness-<版本>` 重建，改 `desktop` 版本号三处后也可整包）。

可用脚本 `desktop/scripts/apply-customizations.sh <官方新树>` 自动执行 A 段复制 + 校验 B 段文件存在性，见该脚本头部说明。
