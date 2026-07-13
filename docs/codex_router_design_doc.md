# Codex 第三方模型智能路由系统设计文档（V1/V2）

## 项目背景

通过本地 Router + Marketplace Catalog 注入，实现 Codex Desktop GUI 同时显示官方模型与第三方模型。

## 核心架构

```text
Codex Desktop
    ↓
Local Router
    ↓
Third Party API
```



## 技术栈

| 层       | 技术                                                         |
| -------- | ------------------------------------------------------------ |
| 应用壳   | [Tauri 2](https://v2.tauri.app/)                             |
| 前端     | [React 18](https://react.dev/) + TypeScript + [Vite 6](https://vite.dev/) |
| 样式     | [Tailwind CSS 3](https://tailwindcss.com/) + [shadcn/ui](https://ui.shadcn.com/) |
| 状态管理 | [TanStack Query](https://tanstack.com/query)                 |
| 原生层   | [Rust](https://www.rust-lang.org/)（Tauri commands）         |
| 国际化   | [i18next](https://www.i18next.com/) + react-i18next          |

参考项目: https://github.com/borawong/AiMaMi



## V1 方案（推荐）

### 核心思想

一个统一 Router：

- 一个 model_provider
- 一个 profile
- 多个 catalog models

### config.toml

```toml
profile = "custom"

model_catalog_json = "C:\\Users\\14128\\.codex\\codexmate\\relay\\codex_router_catalog.json"

[model_providers.custom]
name = "AI智能路由"
base_url = "http://127.0.0.1:25817/codex/router/v1"
wire_api = "responses"
requires_openai_auth = true

[profiles.custom]
model_provider = "custom"
model = "default_model"
```

### catalog 示例

```json
{
  "models": [
    {
      "slug": "test123",
      "display_name": "GPT5.5 中转"
    },
    {
      "slug": "deepseek456",
      "display_name": "DeepSeek Pro"
    }
  ]
}
```

### Router Mapping

```json
{
  "test123": {
    "baseUrl": "http://aaaa.com/v1",
    "apiKey": "xxx",
    "realModel": "gpt-5.5"
  }
}
```

### 请求流程

```text
GUI选择模型
    ↓
Codex发送 model=test123
    ↓
Router收到
    ↓
test123 -> gpt-5.5
    ↓
转发第三方API
```

## V2 方案（规划）

### 核心思想

每个模型：

- 自动生成 model_provider
- 自动生成 profile

### 示例

```toml
[model_providers.deepseek]
base_url = "http://aaaa.com/v1"
wire_api = "responses"

[profiles.deepseek]
model_provider = "deepseek"
model = "deepseek-pro"
```

### V2 用途

- CLI profile 兼容
- 多协议支持
- 多认证体系
- 独立 provider 能力

## 推荐结论

当前推荐：

```text
统一 Router
+
统一 Provider
+
统一 Profile
```

V2 作为后续扩展。
