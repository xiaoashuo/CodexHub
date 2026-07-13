# AGENTS.md

## 项目概览

这个仓库是 `codex-proxy`，一个面向 Codex Desktop 的本地桌面管理工具。它使用 Tauri 2 + Rust 提供本地能力，使用 Vite + React + TypeScript + Tailwind CSS 构建前端界面。

项目的核心目标是让用户在本机管理 Codex 相关能力，包括第三方模型路由、模型配置、账号快照、会话扫描/修复、MCP 管理、技能管理、维护工具和本地日志。应用会启动一个本地 Router，将 Codex 请求转发到配置好的模型 Provider，并同步 Codex 的模型目录与配置。

## 主要功能

- 模型管理：维护第三方 Provider、真实模型名、协议类型、端点路径、代理模式和启用状态。
- 路由管理：启动、停止、重启和检查本地 Router，默认监听 `127.0.0.1:25817`。
- Codex 配置注入：按需写入用户目录下的 `.codex/config.toml` 和模型 catalog，使 Codex 通过本地 Router 访问模型。
- 会话管理：扫描 `.codex/sessions` 和 `.codex/archived_sessions`，展示项目分组、索引缺失和可恢复状态。
- 账号管理：管理 Codex 账号快照、OAuth 登录、使用量刷新和账号切换。
- MCP 与技能管理：查看、导入、启用、禁用和移除本机 Codex MCP/Skills 配置。
- 维护工具：查看本地配置路径、日志、缓存、备份和运行状态。

## 技术栈

- 前端：React、TypeScript、Vite、Tailwind CSS、lucide-react。
- 桌面壳：Tauri 2。
- 原生层：Rust，Tauri commands，内置 TCP HTTP Router。
- 配置与状态：用户主目录下的 `.codex`，以及 `.codex/ai-router-workspace` 内的配置、日志、缓存和备份。

## 重要目录

- `src/`：React 前端入口、页面、组件、类型和 Tauri bridge。
- `src/features/`：业务页面，包括账号、模型、路由、会话、MCP、技能、维护和设置。
- `src/components/`：通用 UI 组件和业务组件。
- `src/lib/tauriBridge.ts`：前端调用 Rust Tauri commands 的集中封装。
- `src-tauri/src/lib.rs`：Rust 主逻辑，包括 Router、配置读写、Codex 账号/会话扫描和 Tauri commands。
- `docs/`：设计说明和功能规格。
- `public/model-actions/`：模型操作图标资源。
- `dist/`：构建产物，不要手动编辑。
- `node_modules/`：依赖目录，不要手动编辑。

## 常用命令

- 安装依赖：`npm install`
- 前端开发：`npm run dev`
- 前端构建：`npm run build`
- 预览构建：`npm run preview`
- Tauri 开发：`npm run tauri:dev`
- Tauri 构建：`npm run tauri:build`
- Rust 检查：`npm run tauri:check`

修改前端或 Rust/Tauri 逻辑后，优先运行 `npm run build`。只改 Rust 原生层时，也运行 `npm run tauri:check`。

## 开发约定

- 保持前端页面与现有结构一致，优先复用 `src/components/ui` 和 `src/components/business` 中的组件。
- 新增 Tauri command 时，同时更新 `src-tauri/src/lib.rs` 的 `invoke_handler` 和 `src/lib/tauriBridge.ts` 的调用封装。
- TypeScript 类型优先放在 `src/types.ts` 或 `src/lib/appTypes.ts`，避免在多个页面重复定义相同接口。
- Router、账号、会话和配置相关逻辑要保持可追踪日志，但日志中不得记录完整 token、cookie、api key 或用户完整对话内容。
- 对 `dist/`、`node_modules/`、截图、调试脚本和生成文件保持克制，除非任务明确要求，不要修改它们。
- 现有部分文案存在编码异常。改动相关文案时，优先使用清晰的 UTF-8 中文，并确认界面不会出现乱码或布局溢出。

## 安全边界

- 不要读取或展示 `.codex/auth.json` 中的敏感内容。
- 不要在日志、截图、文档或测试输出中泄露 access token、refresh token、cookie、api key、OAuth code 或账号私密信息。
- 会话管理功能默认应以只读扫描为主；涉及删除、恢复索引、移动会话或重启 Codex 的逻辑必须有清晰的用户触发和备份策略。
- 写入用户 `.codex/config.toml` 时，只修改本项目管理的 Router 配置块或明确负责的 top-level Router 键，避免覆盖用户无关配置。
- 项目级 Codex 配置不应保存个人凭据、Provider 密钥或全局偏好；这些应保存在用户本地配置或应用管理的安全位置。

## 验证重点

- Router 能正确响应 `/health`、`/codex/router/v1/responses` 和 `/codex/router/v1/chat/completions`。
- Provider 配置读写、导入导出、模型 catalog 同步后不会丢失启用状态和 active model。
- 账号管理不泄露敏感字段，只展示 mask 后的信息。
- 会话扫描不会修改 `.codex` 会话文件；修复类操作必须保留备份并反馈恢复数量。
- 前端页面在窄宽度和长文本下不应出现按钮文字、表格内容或弹窗内容重叠。

## 指定参考资料

- OpenAI Codex Advanced Configuration：<https://developers.openai.com/codex/config-advanced>
- OpenAI Codex AGENTS.md 指南：<https://developers.openai.com/codex/guides/agents-md>
- 本仓库设计文档：`docs/codex_router_design_doc.md`
- 本仓库会话管理规格：`docs/codex_thread_manager_v1_spec.md`

参考官方 Codex 文档时，优先遵循当前公开文档。高级配置用于理解 `config.toml`、项目配置、Provider、MCP 和 hooks 等 Codex 配置面；`AGENTS.md` 只用于本仓库的持久工作约定、验证命令和安全边界。
