import json
import shutil
import sqlite3
import time
from datetime import datetime, timezone
from pathlib import Path

codex = Path(r"C:\Users\14128\.codex")
db_path = codex / "state_5.sqlite"
state_path = codex / ".codex-global-state.json"
index_path = codex / "session_index.jsonl"
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-swf-sidebar-like-codex-proxy-{int(time.time())}"

project = r"D:\360Downloads\ziliao\ts\swf-eat-record"
project_ext = r"\\?\D:\360Downloads\ziliao\ts\swf-eat-record"


def normalize_path(value: str) -> str:
    value = (value or "").strip()
    if value.startswith(r"\\?\\"):
        return value[4:]
    if value.startswith(r"\\?"):
        return value[4:]
    return value


def unique(items):
    seen = set()
    result = []
    for item in items:
        if item and item not in seen:
            seen.add(item)
            result.append(item)
    return result


def ids_from_order(value):
    if isinstance(value, dict) and isinstance(value.get("threadIds"), list):
        return [item for item in value["threadIds"] if isinstance(item, str)]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, str)]
    return []


def thread_order(ids):
    return {"threadIds": unique(ids)}


def front_unique(current, selected):
    return unique(list(selected) + [item for item in current if item not in selected])


def iso_from_seconds(seconds):
    return datetime.fromtimestamp(int(seconds), tz=timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


backup_dir.mkdir(parents=True, exist_ok=True)
for path in [state_path, index_path, db_path]:
    if path.exists():
        shutil.copy2(path, backup_dir / path.name)

con = sqlite3.connect(db_path)
con.row_factory = sqlite3.Row
rows = con.execute(
    """
    SELECT id,
           COALESCE(NULLIF(title, ''), NULLIF(first_user_message, ''), id) AS title,
           COALESCE(updated_at, created_at, 0) AS updated_at,
           COALESCE(NULLIF(first_user_message, ''), NULLIF(title, ''), id) AS prompt
    FROM threads
    WHERE COALESCE(archived, 0) = 0
      AND replace(cwd, '\\\\?\\', '') = ?
    ORDER BY COALESCE(updated_at_ms, updated_at * 1000) DESC, id ASC
    """,
    (project,),
).fetchall()
swf_ids = [row["id"] for row in rows]
if not swf_ids:
    raise SystemExit(f"no sqlite threads found for {project}")

# 和可正常显示的新线程保持一致：修正 NULL namespace。
con.execute(
    "UPDATE thread_dynamic_tools SET namespace = 'codex_app' WHERE thread_id IN (%s) AND namespace IS NULL"
    % ",".join(["?"] * len(swf_ids)),
    swf_ids,
)
con.commit()
con.close()

state = json.loads(state_path.read_text(encoding="utf-8", errors="ignore"))
if not isinstance(state, dict):
    state = {}

# 1) workspace root 列表里只保留普通路径，并把 swf 放前面用于测试。
for key in ["active-workspace-roots", "electron-saved-workspace-roots", "project-order"]:
    current = state.get(key)
    if not isinstance(current, list):
        current = []
    normalized = unique([normalize_path(item) for item in current if isinstance(item, str)])
    state[key] = front_unique(normalized, [project])

# 2) 项目分组里明确放入 swf 的全部线程，顺序按 sqlite updated desc。
orders = state.setdefault("sidebar-project-thread-orders", {})
if not isinstance(orders, dict):
    orders = {}
    state["sidebar-project-thread-orders"] = orders
normal_order = ids_from_order(orders.get(project))
variant_order = ids_from_order(orders.pop(project_ext, None))
orders[project] = thread_order(front_unique(normal_order + variant_order, swf_ids))

# 3) 全局 chat 顺序也提前 swf 全部线程；Codex UI 很可能先加载 chat，再做项目归组。
state["sidebar-chat-thread-order"] = thread_order(front_unique(ids_from_order(state.get("sidebar-chat-thread-order")), swf_ids))

# 4) workspace hints / prompt-history 补齐。
hints = state.setdefault("thread-workspace-root-hints", {})
if not isinstance(hints, dict):
    hints = {}
    state["thread-workspace-root-hints"] = hints
for tid in swf_ids:
    hints[tid] = project

for atom_key in ["persisted-atom-state", "electron-persisted-atom-state"]:
    atom = state.setdefault(atom_key, {})
    if not isinstance(atom, dict):
        atom = {}
        state[atom_key] = atom
    atom["sidebar-workspace-filter-v2"] = "all"
    atom["sidebar-organize-mode-v1"] = "project"
    atom["sidebar-history-show"] = "all"
    atom["sidebar-history-organize"] = "project"
    atom["thread-sort-key"] = "updated_at"
    atom["sidebar-move-updated-threads-in-front-v1"] = True
    collapsed = atom.setdefault("sidebar-collapsed-groups", {})
    if isinstance(collapsed, dict):
        collapsed.pop(project, None)
        collapsed.pop(project_ext, None)
    sections = atom.setdefault("sidebar-collapsed-sections-v1", {})
    if isinstance(sections, dict):
        sections["chats"] = False
        sections["pinned"] = False
        sections["threads"] = False
    prompt_history = atom.setdefault("prompt-history", {})
    if isinstance(prompt_history, dict):
        for row in rows:
            prompt_history[row["id"]] = [row["prompt"]]

state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

# 5) session_index 也把 swf 全部线程提前，模拟 codex-proxy 的加载效果。
existing = []
seen = set()
if index_path.exists():
    for line in index_path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            item = json.loads(line)
        except Exception:
            continue
        tid = item.get("id")
        if isinstance(tid, str) and tid and tid not in seen and tid not in swf_ids:
            seen.add(tid)
            existing.append(item)

swf_index_items = [
    {
        "id": row["id"],
        "thread_name": row["title"],
        "updated_at": iso_from_seconds(row["updated_at"]),
    }
    for row in rows
]
next_items = swf_index_items + existing
index_path.write_text("\n".join(json.dumps(item, ensure_ascii=False, separators=(",", ":")) for item in next_items) + "\n", encoding="utf-8")

print(f"backup={backup_dir}")
print(f"project={project}")
print(f"promoted_count={len(swf_ids)}")
print("first_ids=" + ",".join(swf_ids[:8]))
