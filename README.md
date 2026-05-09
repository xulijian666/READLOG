# ReadLog 日志工具

一个基于 Tauri 2 + React + Rust 的 Windows 桌面工具，用来维护内网日志 URL、下载日志文件，并对已配置的单体日志文件做快速关键词查询。

## 主要功能

### 日志 URL 配置

- 统一配置 Basic Auth 用户名和密码。
- 密码使用 Windows DPAPI 加密后保存到本机配置文件。
- 支持维护多条日志 URL，每条配置由 `baseUrl + path + logFile` 组成。
- 支持按分组管理日志，例如 `SIT-C1`、`SIT-C2`、`SIT-C3`。
- 支持显示/隐藏日志项、勾选/取消勾选日志项。
- 支持按日志名称或路径搜索，快速勾选相关日志。
- 支持测试已配置日志 URL 的连接状态和文件大小。
- 支持从 Excel 导入日志配置，也支持导出当前日志配置到 Excel。

配置文件位置：

```text
%APPDATA%/LogViewer/config.json
```

### 下载日志

下载功能只针对当前勾选的日志配置。

- 实时日志下载：下载每条配置里的当前 `logFile`，例如 `app.log`。
- 归档日志下载：按月份、日期、小时范围下载归档日志。
- 尾部截取：下载每个日志文件末尾指定行数。
- 支持 `.gz` 日志自动解压。
- 支持选择下载目录。
- 下载完成后可以打开文件、打开所在文件夹。
- 支持复制分析提示词，方便把下载后的日志交给外部工具继续分析。

### 查询日志

查询日志是轻量的“极速 grep UI”，只围绕当前已经配置并勾选的单体日志 URL 工作。

- 不扫描目录。
- 不自动推导历史文件。
- 不做链路分析。
- 不做日志时间解析。
- 只对当前勾选的日志文件 URL 做关键词搜索。

查询能力：

- 输入关键词后，对已勾选日志并发查询。
- 普通 `.log` 使用 HTTP 流式读取，边下载边按行匹配。
- 非命中行不会长期保留，命中后只缓存必要上下文。
- 默认展示命中行前后各 5 行。
- 点击结果可展开更大上下文，当前默认前后各 200 行。
- 支持三种匹配方式：
  - 完整匹配：按输入内容整体匹配，适合查 traceId、订单号、完整接口路径。
  - 包含全部：按空格拆分关键词，同一行必须全部包含，适合查 `aaa` 且 `bbb`。
  - 包含任一：按空格拆分关键词，同一行包含任意一个即可，适合查 `aaa` 或 `bbb`。
- 支持区分大小写。
- 支持设置最大结果数，默认 500，避免结果过多拖慢界面。
- 支持取消正在进行的查询。

当前 `.gz` 搜索会先下载并解压后再按行匹配，后续可继续优化为 gzip 流式读取。

### 兼容性处理

- 配置文件兼容旧版 `servers` 字段。
- 读取配置时兼容 UTF-8 BOM，避免 `json error: expected value at line 1 column 1`。

## 技术架构

- 前端：React 19 + TypeScript + Tailwind CSS
- 桌面框架：Tauri 2
- 后端：Rust
- 状态管理：Zustand
- Excel 导入：SheetJS
- Excel 导出：rust_xlsxwriter
- HTTP 下载和查询：reqwest

核心模块：

- `src/App.tsx`：主界面和“下载日志 / 查询日志”板块切换。
- `src/components/ServerConfig.tsx`：认证、URL 配置、分组、连接测试。
- `src/components/DownloadPanel.tsx`：实时日志、归档日志、尾部截取。
- `src/components/SearchPanel.tsx`：查询日志界面。
- `src/components/LogPathModal.tsx`：日志路径配置、Excel 导入导出。
- `src-tauri/src/config.rs`：配置读写、旧配置兼容、密码处理。
- `src-tauri/src/download.rs`：日志下载、归档下载、尾部截取。
- `src-tauri/src/search.rs`：流式搜索、上下文缓存、查询取消。
- `src-tauri/src/directory.rs`：目录读取、连接测试。
- `src-tauri/src/main.rs`：Tauri 命令入口。

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

如果打包时报 `read-log.exe` 被占用，先关闭正在运行的日志工具窗口，再重新执行打包命令。
