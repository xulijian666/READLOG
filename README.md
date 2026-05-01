# 日志下载器

一个基于 Tauri + React 的 Windows 桌面工具，用于从多个内网日志 URL 下载 `.log` 文件，并按勾选顺序合并成一个本地日志文件。

## 功能

- 统一配置 Basic Auth 用户名和密码
- 维护多条日志文件 URL
- 只允许配置 `.log` 文件 URL
- 一键测试所有 URL 连接状态
- 测试连接时显示每个日志文件大小
- 勾选需要下载的日志
- 一键合并下载到系统 Downloads 目录
- 下载结果按来源 URL 添加分隔头

## 架构

- 前端：React 19 + TypeScript + TailwindCSS
- 桌面框架：Tauri 2
- 后端：Rust
- 配置存储：`%APPDATA%/LogViewer/config.json`
- 密码存储：Windows DPAPI 加密后写入配置文件

核心模块：

- `src/`：React 前端界面
- `src/components/ServerConfig.tsx`：认证、URL 配置、连接测试
- `src/components/DownloadPanel.tsx`：合并下载入口
- `src-tauri/src/config.rs`：配置读写、URL 校验、密码加密
- `src-tauri/src/directory.rs`：连接测试、文件大小读取
- `src-tauri/src/download.rs`：日志下载与合并写入
- `src-tauri/src/main.rs`：Tauri 命令入口

## 开发命令

安装依赖：

```bash
npm install
```

启动前端预览：

```bash
npm run dev
```

运行 Rust 测试：

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

构建前端：

```bash
npm run build
```

打包 exe / 安装包：

```bash
npm run tauri build
```

## 构建产物

打包成功后主要产物在：

```text
src-tauri/target/release/read-log.exe
src-tauri/target/release/bundle/nsis/LogDownloader_0.1.0_x64-setup.exe
src-tauri/target/release/bundle/msi/LogDownloader_0.1.0_x64_en-US.msi
```

如果打包时报 `read-log.exe` 被占用，先关闭正在运行的日志下载器窗口，再重新执行打包命令。
