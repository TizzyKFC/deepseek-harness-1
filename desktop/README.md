# DeepSeek Harness macOS 桌面版（Tauri）

把本地构建的 `deepseek-harness` 打包成**完全自包含**的 macOS 应用：一个原生窗口运行 `dsh web`（浏览器界面），放在 Dock 里使用，无需打开浏览器。

- 技术栈：Tauri 2（Rust 壳）+ 项目自带的 Node + pnpm（均随包内置）
- 产物：`DeepSeek Harness.dmg`（约 480MB，仅支持 Apple Silicon / arm64）
- 自包含：应用内携带完整构建产物与 Node 运行时，首次启动自动解压，不依赖本地开发环境

---

## 安装

1. 双击 `DeepSeek Harness.dmg`
2. 把 `DeepSeek Harness.app` 拖入 `Applications`
3. 首次打开：由于未做 Apple 签名，需 **右键 → 打开**（或在「系统设置 → 隐私与安全性」中允许）
4. 应用出现在 Dock 中，启动后显示 DeepSeek Harness 界面

## 首次启动

- 首次启动会把内置的 harness（约 1.5GB 源码+依赖）解压到应用数据目录，耗时十几秒到几十秒，之后秒开
- 数据目录：`~/Library/Application Support/com.deepseek-ai.harness.desktop/`
  - `harness-<版本>/` — 解压出的项目（按 app 版本隔离：同版本带 `.extracted` 标记直接复用，升版本自动重新解压并清理旧目录）
  - `dsh/` — 应用的 DSH_HOME（profile、会话、设置都在这）
  - `dsh/server.log` — 服务器诊断日志
  - `update-check.log` / `update-check.json` / `update-notice.json` — 启动更新检查的日志 / 去重状态 / 结构化结果
- 每次启动自动运行 `dsh web --port 0`（端口由系统分配，仅监听 `127.0.0.1`），关闭应用自动停止

## 内置主题（Blue Fantasy 切换）

- 应用内置 **Blue Fantasy（鲸鱼娘）** 主题作为默认外观，无需安装任何皮肤插件。
- 左侧导航栏有 **主题** 按钮，可在 Blue Fantasy 与默认主题之间即时切换；选择持久保存在 `dsh/settings.yaml` 的 `skin.theme` 段，重启后保持。
- 该功能由内置插件 `@deepseek-ai/dsh-client-ui-skin-toggle` 提供，皮肤资产（样式、鲸鱼背景、图标）已内置、完全自包含。

## 启动更新检查

每次启动 app 会在后台检查官方 `deepseek-ai/deepseek-harness` 是否有新版本 tag：

- 发现新版 → 弹出**版本对话框**（显示 当前版本 → 最新版本，可一键跳转 GitHub 发布页），并写 `update-notice.json`；同一版本只提醒一次。
- 仅提醒，**不自动下载/覆盖**；升级方式见文末「跟随官方升级」。

## 配置 API Key

应用需要 DeepSeek API Key 才能工作。二选一：

- 设置环境变量 `DEEPSEEK_API_KEY`（应用继承启动环境）
- 写入凭据文件 `~/Library/Application Support/com.deepseek-ai.harness.desktop/dsh/.credentials.yaml`：

  ```yaml
  deepseek_api_key: sk-xxxx
  ```

> 凭据解析顺序见项目文档 `docs/`（环境变量 → `.credentials.yaml` → `.env`）。

## 在应用内安装插件

菜单栏 → **插件 → 插件管理器…**（快捷键 `⌘⇧M`）打开插件管理器窗口：

- **安装**：输入包名（npm 包 / git 地址 / 绝对路径），点「安装」
- **卸载**：输入已安装的包名，点「卸载」
- **更新全部**：更新 profile 内所有依赖
- **重启应用**：插件作为 bundle 层在启动时加载，安装后需重启生效
- 窗口会显示 pnpm 的输出；已安装依赖与激活的 bundle 层实时列出

插件安装到应用的 profile：`…/dsh/profiles/web/`，与应用数据同存，升级应用不丢失。

> 说明：git 依赖被 pnpm 阻止构建时，按输出提示在 profile 的 `pnpm-workspace.yaml` 加入 `allowBuilds` 后重试。
> harness 内置插件（`@deepseek-ai/dsh-base` 等）已包含，无需也不应重复安装。

## 重新打包（开发者）

```sh
# 1. 在仓库根目录先构建项目（产物会被打进 app）
pnpm install
pnpm run build

# 2. 打包 DMG（staging 会重新生成：harness.tar.gz + node + pnpm）
cd desktop
npm install        # 安装 @tauri-apps/cli（仅首次）
npm run dmg        # = prep-bundle.sh + tauri build --bundles dmg
```

产物：`desktop/src-tauri/target/release/bundle/dmg/DeepSeek Harness_<版本>_aarch64.dmg`

可选环境变量：`DSH_DESKTOP_NODE_VERSION`（默认 26.7.0）、`DSH_DESKTOP_PNPM_VERSION`（默认 11.7.0）、
`HARNESS_SRC`（打进 DMG 的 harness checkout，默认本仓库根；适配别的官方基线时指向该官方树，如 `HARNESS_SRC=/path/to/官方0.1.3树 bash scripts/prep-bundle.sh`）。

> 版本三处同步：`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`、`package.json`。
> 升版本后**删除**数据目录里的 `harness-<旧版本>`，否则同版本会复用旧解压而不重新解压。

## 结构

```
desktop/
├── package.json            # npm 项目（与 pnpm workspace 隔离），@tauri-apps/cli
├── ui/
│   ├── index.html          # 启动加载页（等待服务器就绪）
│   └── plugin-manager/     # 插件管理器窗口页面（Tauri IPC → Rust 命令）
├── scripts/prep-bundle.sh  # 生成 staging：项目 tar 归档 + Node + pnpm
└── src-tauri/
    ├── src/lib.rs          # Rust 壳：启停服务器、导航、菜单、插件管理命令
    ├── tauri.conf.json
    └── staging/            # 打包内容（gitignore，由 prep 生成）
```

### 设计要点

- **符号链接环**：pnpm 的 `node_modules` 含符号链接环，Tauri 资源复制会跟随链接而失败；因此项目以 **tar 归档** 打包（tar 不跟随链接、完整保留链接环），首次启动用系统 `tar` 解压。
- **插件管理**：Rust 壳调用 harness 自带的 `dsh plugin --profile web` 命令，并把内置 pnpm/node 的 bin 目录前置到 PATH，复用 harness 的 profile 初始化与 bundle 调和逻辑。
- **安全**：服务器仅监听 `127.0.0.1`，不对外暴露。

## 跟随官方升级

- **当前 shipped 基线 = 官方 `0.1.3-alpha.2`（直接适配，已端到端验证）**，定制明细见仓库根 `CUSTOMIZATIONS.md`。
  0.1.3 适配全在 `desktop/`：`lib.rs` 的 `--no-open` + 认证握手（0.1.3 用 `?token=` 换 HttpOnly
  cookie；WKWebView 顶层导航不保留 303 的 Set-Cookie，改为在 401 页内同源 `fetch` 换 cookie 后重载 `/`）；
  `prep-bundle.sh` 新增 `HARNESS_SRC` 指向官方树。
- **升级 harness 内部（不必重打包壳）**：取官方新树 → 新树内 `pnpm install && pnpm run build`
  → `desktop/scripts/sync-harness.sh`（自动退出 app、归档运行所需并重建 `data_dir/harness-<版本>`，
  自动排除官方测试/文档/示例；用户数据在 `dsh/` 不受影响）→ 重启 app。
- **换新基线并连壳一起出新 DMG**：`cd desktop && HARNESS_SRC=<官方新树> bash scripts/prep-bundle.sh && npx tauri build --bundles dmg`（见「重新打包」）。
- 旧的 0.1.0 定制重放流程（`apply-customizations.sh`、`ui-skin-toggle` 皮肤 6 处合并）见
  `CUSTOMIZATIONS.md` A/B 段；皮肤在 0.1.3 上重写落地前仅供历史参考。
