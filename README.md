# Codex 伴侣 / codex-proxy

`codex-proxy` 是一个面向 [Codex Desktop](https://developers.openai.com/codex/) 的本地桌面管理工具，帮你在一处统一管理第三方模型路由、账号快照、会话索引、MCP、Skills 以及本地维护任务。

它基于 Tauri 2 + Rust 提供本地能力，前端使用 Vite + React + TypeScript + Tailwind CSS 构建。应用会启动一个本地 Router（默认监听 `127.0.0.1:25817`），将 Codex 请求转发到你配置的第三方模型 Provider，让你在 Codex 中像使用官方模型一样选择和使用任意模型。

## 功能概览

### 仪表盘

首屏以概览面板展示 Router 运行状态、账号数量、已启用模型数、会话总量、Skills 和 MCP 数量，以及分 Provider 的 Token 用量统计，一眼看清整体情况。

### 模型管理

- 新增、编辑、删除第三方模型 Provider 配置，支持配置显示名、真实模型名、Base URL、API Key、协议类型（OpenAI / Anthropic / Google 等）、端点路径、上下文窗口和代理模式。
- 一键启用 / 禁用模型，设置当前 active 模型。
- 模型连通性测试和聊天测试，快速验证连接状态。
- 支持导入 / 导出 Provider 配置。
- 一键将已启用模型同步到 Codex Model Catalog，模型即可在 Codex 界面中直接选择使用。

### 路由管理

- 启动、停止、重启本地 Router，默认地址 `http://127.0.0.1:25817/codex/router/v1`。
- 健康检查地址 `http://127.0.0.1:25817/health`。
- 支持请求路径：`/codex/router/v1/responses` 和 `/codex/router/v1/chat/completions`。
- 启动前自动执行配置写入、会话索引恢复、端口占用检查及 Codex 重启处理等准备工作。
- 查看和清理 Router 请求日志。

### Codex 配置注入

- 写入用户目录下 `.codex/config.toml` 中由本项目管理的 Router 配置块。
- 生成并维护本地模型 Catalog，让 Codex Desktop 能够识别并展示已启用的第三方模型。
- 写入时尽量只修改 Router 相关配置，避免覆盖用户的其他 Codex 配置。

### 账号管理

- 扫描和管理 Codex 本地账号快照，支持导入当前账号、OAuth 登录和 ChatGPT Session 导入。
- 账号切换、删除快照、导出账号数据。
- 自动刷新账号使用量，可配置刷新间隔。
- 内置账号反向代理，可将本地账号能力暴露为兼容 API 接口。
- 所有敏感字段（token、cookie、API key）均进行脱敏处理，界面只展示遮蔽后的信息。

### 会话管理

- 扫描机器下的本地 Codex 会话。
- 按项目分组展示会话数量、大小、活跃天数和索引状态。
- 识别缺失索引、已归档会话和解析异常，支持检查与恢复会话索引（恢复前自动备份）。
- 支持删除指定会话文件，由用户明确触发。

### MCP 管理

- 从 Codex `config.toml` 读取 MCP Server 配置并集中展示。
- 新增、编辑、启用、禁用和移除 MCP Server。
- 维护 MCP 配置时尽量保留用户其他配置内容。

### 技能管理

- 扫描本机 Codex Skills 安装目录，展示已安装的技能列表。
- 导入 Skill、移除 Skill 并在操作前自动创建备份，方便随时回滚。
- 查看备份、恢复备份和删除备份。

### 维护工具

- 一键查看本地配置路径、日志路径、Provider 配置路径和 Catalog 路径。
- 搜索、预览和清理应用日志和 Router 日志。
- 清理维护缓存和调试数据。
- 创建和导入迁移备份，支持配置迁移。
- 检查新版本，下载安装包并启动安装。

### 设置

- 配置 Router 端口、Codex 可执行文件路径、账号代理地址和 OAuth 回调端口。
- 设置 Router 并发上限。
- 自定义 Router 启动时 Codex 重启行为。

## 技术栈

| 模块     | 技术                                       |
| -------- | ------------------------------------------ |
| 桌面壳   | Tauri 2                                    |
| 原生层   | Rust、Tauri commands、本地 TCP HTTP Router |
| 前端     | React、TypeScript、Vite                    |
| 样式     | Tailwind CSS                               |
| 图标     | lucide-react                               |
| 配置目录 | `~/.codex`、`~/.codex/ai-router-workspace` |

## 项目结构

```text
src/
  App.tsx                        前端主入口和页面编排
  components/                    通用 UI 与业务组件
  features/                      账号、模型、路由、会话、MCP、技能、维护、设置等页面
  lib/tauriBridge.ts             前端调用 Tauri commands 的封装
  types.ts                       前端共享类型

src-tauri/
  src/lib.rs                     Rust 主逻辑、Router、配置读写、账号和会话扫描
  src/main.rs                    Tauri 启动入口
  tauri.conf.json                Tauri 应用配置

docs/                            设计文档与规格说明
scripts/                         会话修复、Catalog 同步等辅助脚本
```

## 本地运行

安装依赖：

```bash
npm install
```

启动前端开发服务：

```bash
npm run dev
```

启动 Tauri 开发模式：

```bash
npm run tauri:dev
```

构建前端：

```bash
npm run build
```

检查 Rust/Tauri 原生层：

```bash
npm run tauri:check
```

构建桌面安装包：

```bash
npm run tauri:build
```



## 安全说明

- 不读取、展示或记录 `.codex/auth.json` 中的敏感内容。
- 日志、截图、文档和测试输出中不会泄露 access token、refresh token、cookie、API key、OAuth code 或账号私密信息。
- 账号、Provider 和 Router 日志均对敏感字段进行脱敏处理。
- 会话扫描默认以只读为主；涉及删除、恢复索引、移动会话或重启 Codex 的操作必须由用户明确触发，并在修复操作中保留备份。
- 写入 Codex 配置时只修改本项目负责的 Router 配置块，避免覆盖用户无关配置。
- 所有请求不经过三方中转,全从客户机本地触发。

## License

[Apache License 2.0](LICENSE)
