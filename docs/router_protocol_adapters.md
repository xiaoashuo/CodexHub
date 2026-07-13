# Router 多协议适配与 Tool Call 转换说明

## 三种端点的协议差异

Router 同时暴露三条上游协议路径（account proxy），以及一条内部 `/codex/router/v1/responses` 路径（Codex Desktop 直连）。它们各自使用完全不同的消息格式和 tool call 表示法。

### 消息格式对比

| 特性                | Codex Responses 格式              | OpenAI Chat Completions 格式      | Anthropic Messages 格式           |
| ------------------- | --------------------------------- | --------------------------------- | --------------------------------- |
| 端点                | `/v1/responses`                   | `/v1/chat/completions`            | `/v1/messages`                    |
| 消息容器            | `input` 数组（typed items）       | `messages` 数组（role-based）     | `messages` 数组（content-block）  |
| 系统提示            | `instructions` 顶层字段           | `messages[0]` role=system         | `system` 顶层字段 / content block |
| 用户消息            | `{role: "user", content: "..."}`  | `{role: "user", content: "..."}`  | `{role: "user", content: [{type: "text", text: "..."}]}` |
| 助手消息            | `{role: "assistant", content}`    | `{role: "assistant", content}`    | `{role: "assistant", content: [{type: "text", text: "..."}]}` |
| 并行 tool calls     | N 个独立的 `{type: "function_call"}` item | 1 条 assistant 消息含 `tool_calls: [A, B, C]` | 1 条 assistant 消息 content 含多个 `{type: "tool_use"}` block |
| 单个 tool call      | 1 个 `{type: "function_call"}` item | 1 条 assistant 消息含 `tool_calls: [A]` | 1 条 assistant 消息 content 含 `{type: "tool_use"}` block |
| tool 调用结果       | `{type: "function_call_output", call_id, output}` | `{role: "tool", tool_call_id, content}` | `{role: "user", content: [{type: "tool_result", tool_use_id, content}]}` |

### 关键差异：并行 tool call 的表示

Codex Responses 格式把并行 tool calls 表示为 `input` 数组中**连续多个独立的 item**：

```json
{
  "input": [
    {"role": "user", "content": "读这两个文件"},
    {"type": "function_call", "call_id": "call_1", "name": "read_file", "arguments": "{\"path\":\"a.txt\"}"},
    {"type": "function_call", "call_id": "call_2", "name": "read_file", "arguments": "{\"path\":\"b.txt\"}"},
    {"type": "function_call_output", "call_id": "call_1", "output": "content of a"},
    {"type": "function_call_output", "call_id": "call_2", "output": "content of b"},
    {"role": "user", "content": "继续"}
  ]
}
```

OpenAI Chat Completions 格式要求并行 tool calls 必须**合并进同一条 assistant 消息**的 `tool_calls` 数组中：

```json
{
  "messages": [
    {"role": "user", "content": "读这两个文件"},
    {"role": "assistant", "content": null, "tool_calls": [
      {"id": "call_1", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"a.txt\"}"}},
      {"id": "call_2", "type": "function", "function": {"name": "read_file", "arguments": "{\"path\":\"b.txt\"}"}}
    ]},
    {"role": "tool", "tool_call_id": "call_1", "content": "content of a"},
    {"role": "tool", "tool_call_id": "call_2", "content": "content of b"},
    {"role": "user", "content": "继续"}
  ]
}
```

如果未合并，每个独立 function_call 会变成一条只有 1 个 tool_calls 的 assistant 消息，DeepSeek（及所有标准 OpenAI 兼容 API）会要求"每条 assistant 的 tool_calls 必须紧跟着所有对应的 tool 消息"，于是抛出 `insufficient tool messages following tool_calls message`。

---

## 完整路由拓扑

```
请求进入 Router
  │
  ├─ /codex/router/v1/responses (Codex Desktop 直连)
  │     │
  │     ├─ 非 custom model → 透传 OpenAI 官方 responses 端点
  │     │
  │     └─ custom model → forward_custom_responses_request
  │            │
  │            ├─ openai/other  → build_openai_chat_body        ← extract_chat_messages(payload, false)
  │            │                    ├─ 有 input 字段 → for-loop 逐个归一化（👈 新增并行合并逻辑）
  │            │                    └─ 有 messages 字段 → filter_map + normalize_chat_message
  │            │
  │            ├─ anthropic     → build_anthropic_messages_body ← extract_chat_messages(payload, true)
  │            │                    （同上，但 omit_system=true，不传 tools 字段）
  │            │
  │            └─ cpamc         → 透传 responses 格式（替换 model）
  │
  └─ /v1/* (Account Proxy)
       │
       ├─ /v1/responses
       │     └─ 直接透传给 OpenAI 官方 Codex responses → forward_official_codex_responses_request
       │
       ├─ /v1/chat/completions
       │     └─ account_proxy_chat_completions_request
       │          ├─ build_responses_payload_from_chat_completions（chat → responses 转换）
       │          │     └─ extract_chat_messages(payload, true)
       │          │           └─ 只有 messages 数组（无 type 字段），走 normalize_chat_message
       │          │              👈 normalize_chat_message 现在保留 tool_calls/tool_call_id/name
       │          ├─ 发给官方 Codex responses
       │          └─ 响应转回 chat completions 格式
       │
       └─ /v1/messages
             └─ account_proxy_messages_request
                  └─ build_responses_payload_from_anthropic_messages（Anthropic → responses）
                       └─ normalize_anthropic_message_for_responses  ← 完全独立，不经过 extract_chat_messages
```

---

## 转换引擎详解

### `extract_chat_messages` — 核心转换引擎

将 Codex responses 的 `input` 数组（或 OpenAI chat 的 `messages` 数组）统一转为标准 chat 消息序列。

```text
extract_chat_messages(payload, omit_system)
  │
  ├─ 有 "messages" 字段（已是 chat 格式）
  │     └─ filter_map → normalize_chat_message(message, omit_system)
  │           system 消息在 omit_system=true 时直接丢弃
  │           👈 现在保留 tool_calls / tool_call_id / name 字段
  │
  └─ 无 "messages" 字段（Codex responses 格式）
        │
        ├─ 提取 instructions → system 消息（omit_system=true 时跳过）
        │
        └─ 遍历 input 数组
              ├─ type == "function_call"  → 攒到 pending_function_calls
              │                              👈 新增：不立即出仓，等连续调用收集完
              ├─ type == "function_call_output"
              │     └─ normalize_responses_function_call_output
              │           → {role: "tool", tool_call_id, content}
              │
              └─ 其他项（role-based）
                    └─ normalize_chat_message(item, omit_system)
                    👈 现在保留 tool_calls / tool_call_id / name

            非 function_call 项出现时（包括 function_call_output / 普通消息），
            先 flush pending_function_calls → 合并成单条 assistant{tool_calls:[...]}
            再处理当前项。
```

### `merge_function_calls_into_assistant` — 新增并行合并

将多个 Codex `function_call` item 合并为一条 OpenAI 兼容的 assistant 消息。

```text
输入: [{call_id:"a", name:"read", args:"..."}, {call_id:"b", name:"write", args:"..."}]

输出: {
  "role": "assistant",
  "content": null,
  "tool_calls": [
    {"id": "a", "type": "function", "function": {"name": "read", "arguments": "..."}},
    {"id": "b", "type": "function", "function": {"name": "write", "arguments": "..."}}
  ]
}
```

### `normalize_chat_message` — 单条消息归一化

```text
normalize_chat_message(value, omit_system)
  │
  ├─ type == "function_call"        → normalize_responses_function_call
  │                                      → {role:"assistant", content:null, tool_calls:[1个]}
  │
  ├─ type == "function_call_output" → normalize_responses_function_call_output
  │                                      → {role:"tool", tool_call_id, content}
  │
  └─ 其他（role-based）
       ├─ omit_system && role=="system" → None（丢弃）
       └─ 保留 role + content
          👈 现在额外保留 tool_calls（assistant）、tool_call_id（tool）、name
```

---

## 关于 Anthropic 路径

Anthropic 消息格式的 tool call 使用完全不同的字段名：

```
Anthropic tool_use:    {type:"tool_use", id, name, input}
Chat tool_calls:       {id, type:"function", function: {name, arguments}}
```

由于字段名不兼容，不能直接在两个表示法之间做映射 -- 仅靠重命名字段会丢失结构信息。Router 目前对 Anthropic 路径的处理方式是：

1. **account proxy `/v1/messages`**：通过 `normalize_anthropic_message_for_responses` 将 Anthropic 消息转为 Codex responses `input` item，交给官方 Codex 处理 tool call。不经过 `extract_chat_messages`。

2. **custom model → Anthropic**：`build_anthropic_messages_body` 不转发 `tools` 字段，因此 An​thropic 后端不会发起 tool call。这是现有行为，本次改动未触及。

---

## 本次修复 （2026-06-10）

### 问题

DeepSeek（OpenAI 兼容协议）在处理 Codex 转发的并行 tool calls 时返回 400 错误：

```
insufficient tool messages following tool_calls message
```

### 根因

Codex responses 的 `input` 数组中，并行 tool calls 是多个独立 `function_call` item。`extract_chat_messages` 原来逐个转成独立的 assistant 消息（每条含 1 个 tool_calls），OpenAI API 规范要求同一轮并行调用必须合并在一条消息里。

### 变更

| 位置 | 变更 | 影响范围 |
| ---- | ---- | -------- |
| `extract_chat_messages` input 分支 | 新增 `pending_function_calls` 收集逻辑 + `merge_function_calls_into_assistant` 合并 | Codex responses → OpenAI/Other 协议转换 |
| `merge_function_calls_into_assistant` | 新增函数，将多个 function_call item 合并成一条 assistant(tool_calls:[...]) | 同上 |
| `normalize_chat_message` | 保留 `tool_calls`、`tool_call_id`、`name` 字段（原 `json_chat_message` 只保留 role+content） | OpenAI chat 格式 → responses 回环路径 |

### 兼容性影响

- **OpenAI/Other custom model 路径**：修复目标路径，并行 tool call 从错误变为正确。
- **chat/completions → official Codex 路径**：`normalize_chat_message` 现在保留更多字段，官方 responses 端点会忽略不认识的字段，行为不变或更好。
- **Anthropic 路径**：完全不受影响（独立转换函数，不经过 `extract_chat_messages`）。
- **非 custom model 路径**：直接透传官方，不受影响。
- **cpamc 路径**：透传，不受影响。
