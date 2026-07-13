# Codex 线程管理 V1 软件开发说明

## 1. 背景

Codex Desktop 的本地会话主要保存在用户目录：

```text
C:\Users\{用户名}\.codex
```

其中 `sessions/**/*.jsonl` 是原始会话文件，`.codex-global-state.json`、`session_index.jsonl`、`state_5.sqlite` 更偏向 UI 状态、索引、线程缓存。

V1 目标不是恢复会话，也不修改 Codex 本地数据，只做只读扫描、统计、分组展示，效果参考“线程管理”页面。

## 2. V1 目标

实现一个“线程管理”页面，支持：

- 扫描当前机器 Codex 本地所有会话文件。
- 展示总线程数、总大小、活跃天数、日均线程。
- 按项目分组展示线程。
- 支持主线程列表展开查看线程明细。
- 标记疑似索引缺失线程。
- 全流程只读，不写入 `.codex` 目录。

## 3. V1 非目标

V1 不做：

- 不恢复会话。
- 不修改 `session_index.jsonl`。
- 不修改 `.codex-global-state.json`。
- 不修改 `state_5.sqlite`。
- 不切换账号。
- 不删除、移动、重命名会话文件。
- 不读取或展示 `auth.json`、token、cookie 等敏感信息。

## 4. 数据来源

### 4.1 必读目录

```text
C:\Users\{用户名}\.codex\sessions
C:\Users\{用户名}\.codex\archived_sessions
```

扫描规则：

- 递归扫描 `*.jsonl` 文件。
- 每个 `rollout-*.jsonl` 视为一个线程。
- `sessions` 下为普通线程。
- `archived_sessions` 下为归档线程。

### 4.2 可选读取文件

```text
C:\Users\{用户名}\.codex\session_index.jsonl
C:\Users\{用户名}\.codex\.codex-global-state.json
C:\Users\{用户名}\.codex\accounts\registry.json
C:\Users\{用户名}\.codex\state_5.sqlite
```

用途：

| 文件 | V1 用途 |
|---|---|
| `session_index.jsonl` | 补充线程标题、判断是否已索引 |
| `.codex-global-state.json` | 补充 prompt-history、projectless 线程、workspace hint |
| `accounts/registry.json` | 展示当前账号基本信息 |
| `state_5.sqlite` | V1 可只读检查表结构，暂不依赖 |

## 5. 数据解析

### 5.1 JSONL 会话文件结构

每行是一个 JSON 对象。

重点读取：

```json
{
  "type": "session_meta",
  "payload": {
    "id": "019e58b3-7a72-7ee3-8544-6c7c063b35aa",
    "timestamp": "2026-05-24T06:36:57.843Z",
    "cwd": "D:\\360Downloads\\ziliao\\ts\\swf-video-cut",
    "originator": "Codex Desktop",
    "cli_version": "0.133.0-alpha.1"
  }
}
```

字段说明：

| 字段 | 说明 |
|---|---|
| `payload.id` | 线程 ID |
| `payload.timestamp` | 创建时间 |
| `payload.cwd` | 当时工作目录，用于项目分组 |
| `payload.originator` | 来源，例如 Codex Desktop |
| `payload.cli_version` | Codex 版本 |

### 5.2 提取用户消息

读取：

```json
{
  "type": "response_item",
  "payload": {
    "type": "message",
    "role": "user",
    "content": [
      {
        "type": "input_text",
        "text": "用户输入内容"
      }
    ]
  }
}
```

用于：

- 生成线程标题兜底值。
- 统计消息数量。
- 展示线程摘要。

注意：

- 跳过 `<environment_context>` 开头的系统环境内容。
- 不展示过长内容，建议截断到 80 到 160 字。

### 5.3 标题获取优先级

线程标题按以下优先级获取：

1. `session_index.jsonl` 中的 `thread_name`。
2. `.codex-global-state.json` 中 `electron-persisted-atom-state.prompt-history[threadId]` 的第一条有效输入。
3. 会话 JSONL 中第一条真实用户消息。
4. 文件名或线程 ID。
5. `无标题线程`。

### 5.4 项目分组规则

优先根据 `cwd` 分组。

规则：

| 条件 | 分组名称 |
|---|---|
| `cwd` 为空 | `Unknown Project` |
| `cwd` 位于 `Documents\Codex\YYYY-MM-DD\...` | `对话` |
| 普通项目路径 | 取路径最后一级目录名 |

示例：

| cwd | 项目名 |
|---|---|
| `D:\360Downloads\ziliao\ts\swf-video-cut` | `swf-video-cut` |
| `D:\360Downloads\ziliao\ts\swf-eat-record` | `swf-eat-record` |
| `C:\Users\14128\Documents\Codex\2026-05-24\xxx` | `对话` |
| 空 | `Unknown Project` |

## 6. 数据模型

### 6.1 ThreadSession

```ts
interface ThreadSession {
  id: string;
  title: string;
  filePath: string;
  source: 'sessions' | 'archived_sessions';
  archived: boolean;
  indexed: boolean;
  missingFromIndex: boolean;
  cwd?: string;
  projectName: string;
  originator?: string;
  cliVersion?: string;
  createdAt?: string;
  updatedAt?: string;
  fileSize: number;
  messageCount: number;
  firstUserText?: string;
  parseErrors: number;
}
```

### 6.2 ProjectGroup

```ts
interface ProjectGroup {
  projectName: string;
  cwd?: string;
  threadCount: number;
  totalSize: number;
  activeDays: number;
  sessions: ThreadSession[];
}
```

### 6.3 ScanSummary

```ts
interface ScanSummary {
  totalThreads: number;
  totalSize: number;
  activeDays: number;
  averageThreadsPerDay: number;
  indexedThreads: number;
  missingFromIndex: number;
  archivedThreads: number;
  projectCount: number;
  scannedAt: string;
}
```

## 7. 页面设计

### 7.1 页面名称

```text
线程管理
```

### 7.2 页面布局

整体结构：

```text
左侧导航
  仪表盘
  账号管理
  线程管理
  中转注入
  MCP 管理
  Skills 管理
  插件管理
  维护工具
  系统设置

右侧主内容
  页面标题
  描述
  统计卡片
  项目 / 主线程 / 子线程列表
```

### 7.3 顶部统计卡片

展示 4 个卡片：

| 卡片 | 计算规则 |
|---|---|
| 总线程数 | 扫描到的 JSONL 会话文件数量 |
| 总大小 | 所有会话文件大小求和 |
| 活跃天数 | 按 `createdAt/updatedAt` 去重日期 |
| 日均线程 | `总线程数 / 活跃天数`，保留 1 位小数 |

示例：

```text
总线程数：41
总大小：41.4 MB
活跃天数：15
日均线程：2.7
```

### 7.4 项目分组列表

每个项目行展示：

- 复选框。
- 展开箭头。
- 项目图标。
- 项目名称。
- 项目路径。
- 线程数量徽标。

示例：

```text
> swf-video-cut
  D:\360Downloads\ziliao\ts\swf-video-cut
  24
```

### 7.5 展开项目后的线程列表

展开后展示线程：

| 字段 | 说明 |
|---|---|
| 标题 | 线程标题 |
| 时间 | 更新时间或创建时间 |
| 大小 | JSONL 文件大小 |
| 消息数 | 用户/助手消息数量 |
| 状态 | 正常、索引缺失、归档、解析异常 |

状态建议：

| 状态 | 条件 |
|---|---|
| 正常 | 文件存在，索引存在 |
| 索引缺失 | 文件存在，但 `session_index.jsonl` 没有该 id |
| 归档 | 文件来自 `archived_sessions` |
| 解析异常 | JSONL 有解析错误 |

## 8. 扫描流程

```text
用户进入线程管理页
        ↓
点击刷新 / 自动扫描
        ↓
读取 Codex Home 路径
        ↓
扫描 sessions + archived_sessions
        ↓
解析每个 JSONL 文件
        ↓
读取 session_index.jsonl
        ↓
读取 .codex-global-state.json 补标题和 workspace hint
        ↓
按 cwd 分组
        ↓
计算统计数据
        ↓
渲染页面
```

## 9. 后端接口建议

### 9.1 扫描线程

```http
GET /api/codex/threads/scan
```

响应：

```json
{
  "summary": {
    "totalThreads": 41,
    "totalSize": 43411000,
    "activeDays": 15,
    "averageThreadsPerDay": 2.7,
    "indexedThreads": 6,
    "missingFromIndex": 33,
    "archivedThreads": 1,
    "projectCount": 5,
    "scannedAt": "2026-05-24T08:00:00Z"
  },
  "projects": [
    {
      "projectName": "swf-video-cut",
      "cwd": "D:\\360Downloads\\ziliao\\ts\\swf-video-cut",
      "threadCount": 24,
      "totalSize": 30000000,
      "activeDays": 8,
      "sessions": []
    }
  ]
}
```

### 9.2 查询项目线程

如果线程很多，可以拆成懒加载接口：

```http
GET /api/codex/threads/projects/{projectKey}/sessions
```

响应：

```json
{
  "projectName": "swf-video-cut",
  "sessions": [
    {
      "id": "019e3a23-84cc-7993-872f-044e257e8628",
      "title": "优化语音识别配置列表",
      "filePath": "C:\\Users\\14128\\.codex\\sessions\\2026\\05\\18\\rollout-xxx.jsonl",
      "source": "sessions",
      "archived": false,
      "indexed": true,
      "missingFromIndex": false,
      "cwd": "D:\\360Downloads\\ziliao\\ts\\swf-video-cut",
      "projectName": "swf-video-cut",
      "createdAt": "2026-05-18T08:11:06Z",
      "updatedAt": "2026-05-18T08:12:13Z",
      "fileSize": 120000,
      "messageCount": 30,
      "firstUserText": "我希望这里作为所有语音服务商列表",
      "parseErrors": 0
    }
  ]
}
```

## 10. 技术实现建议

### 10.1 后端

推荐使用 Node.js 或 Python 均可。

Node.js 适合 Electron/Tauri/前端一体化：

- `fs/promises` 递归扫描文件。
- `readline` 流式读取 JSONL。
- `path` 处理路径。
- `better-sqlite3` 或 `sqlite3` 只读读取 SQLite。

Python 适合快速实现扫描服务：

- `pathlib` 递归扫描。
- 逐行解析 JSONL。
- `sqlite3` 只读检查。

### 10.2 前端

推荐：

- Vue 3 / React 均可。
- 表格或树形列表展示项目和线程。
- 项目默认折叠。
- 点击项目懒加载线程详情。
- 顶部刷新按钮重新扫描。

## 11. 安全要求

必须遵守：

- V1 全部只读。
- 不读取 `auth.json` 内容。
- 不展示 token、cookie、api key。
- 不把完整用户对话上传到远端。
- 默认只展示标题和摘要。
- 读取 `.codex-global-state.json` 时只取必要字段。
- SQLite 使用只读连接。

## 12. 异常处理

| 异常 | 处理 |
|---|---|
| `.codex` 不存在 | 页面提示未检测到 Codex 数据目录 |
| `sessions` 不存在 | 显示 0 线程 |
| JSONL 单行解析失败 | 跳过该行，记录 `parseErrors` |
| 文件被占用 | 跳过并提示部分文件扫描失败 |
| `.codex-global-state.json` 损坏 | 忽略该文件，仅使用 JSONL |
| `session_index.jsonl` 不存在 | 所有线程标记为未索引 |
| SQLite 读取失败 | 不影响 V1 主流程 |

## 13. 验收标准

V1 完成后需要满足：

- 能扫描出本地所有 `sessions/**/*.jsonl`。
- 总线程数与文件数量一致。
- 总大小计算正确。
- 活跃天数按日期去重正确。
- 项目分组与 `cwd` 对应。
- 能识别 `session_index.jsonl` 中不存在的线程。
- 页面刷新不会修改任何 `.codex` 文件。
- 关闭网络也能正常扫描。

## 14. 后续版本

V2：

- 增加“扫描丢失线程”独立列表。
- 支持导出恢复索引草案。
- 仍不写回。

V3：

- 备份后重建 `session_index.jsonl`。
- 小范围 merge `.codex-global-state.json`。

V4：

- 研究并修复 `state_5.sqlite` 中 `threads`、`thread_spawn_edges` 相关索引。

V5：

- 支持账号切换后的本地历史扫描。
- 支持多账号会话视图。
