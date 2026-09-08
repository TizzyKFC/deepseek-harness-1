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
  - `harness/` — 解压出的项目（内置、只读用途）
  - `dsh/` — 应用的 DSH_HOME（profile、会话、日志都在这）
  - `dsh/server.log` — 服务器诊断日志
- 每次启动自动运行 `dsh web --port 0`（端口由系统分配，仅监听 `127.0.0.1`），关闭应用自动停止

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
