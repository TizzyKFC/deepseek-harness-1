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

可选环境变量：`DSH_DESKTOP_NODE_VERSION`（默认 26.7.0）、`DSH_DESKTOP_PNPM_VERSION`（默认 11.7.0）。

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

## 跟随官方升级（保留本地定制）

本仓库 = 官方 `deepseek-ai/deepseek-harness` 源码 + 桌面定制，已用 git 管理并推送
到用户自己的远端。**定制明细、修改点与重放方式见仓库根 `CUSTOMIZATIONS.md`**。

收到更新提醒后，升级 harness 内部（通常不必重打包）：

1. 取官方新 tag 源码树（本仓库已配 `upstream` 指向官方，可 `git fetch upstream` 或下载官方 tar）。
2. 跑 `desktop/scripts/apply-customizations.sh <官方新树>`：自动把 `desktop/` 与
   `packages/client/ui-skin-toggle/` 复制进新树，并列出需手工合并的文件。
3. 按 `CUSTOMIZATIONS.md` 的 B 段合并那几处官方文件的修改（这是唯一需要人工的点）。
4. 在新树内 `pnpm install && pnpm run build`。
5. 一键同步到 app：`desktop/scripts/sync-harness.sh`——自动退出 app、把「运行所需」
   （源码 + 构建产物 + node_modules）归档并重建数据目录 `harness-<版本>`（自动排除
   官方测试/文档/示例，与打包 prep 的排除规则一致），随后重启 app 即可。用户数据在
   `dsh/`，不受影响。

需要连同桌面壳一起更新（改了 `desktop/` 内 Rust/配置）时，才走上面的「重新打包」。
