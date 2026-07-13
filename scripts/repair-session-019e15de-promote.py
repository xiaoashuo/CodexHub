import json
import shutil
import sqlite3
import time
from pathlib import Path

codex = Path(r"C:\Users\14128\.codex")
target_id = "019e15de-9c87-7531-a6c4-ddd7b284982f"
project = r"D:\360Downloads\ziliao\ts\swf-eat-record"
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-restore-promote-019e15de-{int(time.time())}"

state_path = codex / ".codex-global-state.json"
index_path = codex / "session_index.jsonl"
db_path = codex / "state_5.sqlite"


def ids_from_order(value):
    if isinstance(value, dict) and isinstance(value.get("threadIds"), list):
        return [item for item in value["threadIds"] if isinstance(item, str)]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, str)]
    return []


def thread_order(ids):
    seen = set()
    result = []
    for item in ids:
        if item and item not in seen:
            seen.add(item)
            result.append(item)
    return {"threadIds": result}


def front(ids, selected):
    return thread_order(list(selected) + [item for item in ids if item not in selected])


backup_dir.mkdir(parents=True, exist_ok=True)
for path in [state_path, index_path, db_path]:
    if path.exists():
        shutil.copy2(path, backup_dir / path.name)

# 1) 把目标线程提前到 Codex 实际常用的 chat 顺序和项目顺序前面。
state = json.loads(state_path.read_text(encoding="utf-8", errors="ignore"))
state["sidebar-chat-thread-order"] = front(ids_from_order(state.get("sidebar-chat-thread-order")), [target_id])

orders = state.setdefault("sidebar-project-thread-orders", {})
if not isinstance(orders, dict):
    orders = {}
    state["sidebar-project-thread-orders"] = orders
orders[project] = front(ids_from_order(orders.get(project)), [target_id])

hints = state.setdefault("thread-workspace-root-hints", {})
if isinstance(hints, dict):
    hints[target_id] = project

for atom_key in ["persisted-atom-state", "electron-persisted-atom-state"]:
    atom = state.setdefault(atom_key, {})
    if not isinstance(atom, dict):
        atom = {}
        state[atom_key] = atom
    atom["sidebar-workspace-filter-v2"] = "all"
    atom["sidebar-organize-mode-v1"] = "project"
    sections = atom.setdefault("sidebar-collapsed-sections-v1", {})
    if isinstance(sections, dict):
        sections["chats"] = False
        sections["pinned"] = False
        sections["threads"] = False
    collapsed = atom.setdefault("sidebar-collapsed-groups", {})
    if isinstance(collapsed, dict):
        collapsed.pop(project, None)
        collapsed.pop(r"\\?\D:\360Downloads\ziliao\ts\swf-eat-record", None)

state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

# 2) 把 session_index 精确 id 记录提前到第一行，避免 Codex 只加载前 N 条时漏掉。
target_record = None
other_records = []
for line in index_path.read_text(encoding="utf-8", errors="ignore").splitlines():
    if not line.strip():
        continue
    try:
        item = json.loads(line)
    except Exception:
        other_records.append(line)
        continue
    if item.get("id") == target_id:
        target_record = item
    else:
        other_records.append(json.dumps(item, ensure_ascii=False, separators=(",", ":")))

if target_record is None:
    target_record = {
        "id": target_id,
        "thread_name": "您是一名经验丰富的产品和运营人员。\n拥有敏锐的商业嗅觉和逻辑以及产品需求的落地产出。\n参考如下文档 docs\\superpowers\\specs ，了解当前项目 ，给出v2版本建议增加功能计划\n落地文v2文件spec",
        "updated_at": "2026-05-12T02:34:15Z",
    }

next_lines = [json.dumps(target_record, ensure_ascii=False, separators=(",", ":"))] + other_records
index_path.write_text("\n".join(next_lines) + "\n", encoding="utf-8")

# 3) 修正该线程动态工具 namespace 的 NULL 值，和可显示的新线程保持一致。
con = sqlite3.connect(db_path)
try:
    con.execute(
        "UPDATE thread_dynamic_tools SET namespace = 'codex_app' WHERE thread_id = ? AND namespace IS NULL",
        (target_id,),
    )
    con.commit()
finally:
    con.close()

print(f"backup={backup_dir}")
print(f"promoted={target_id}")
