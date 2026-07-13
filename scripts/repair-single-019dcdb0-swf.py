import json
import shutil
import sqlite3
import time
from datetime import datetime, timezone
from pathlib import Path

codex = Path(r"C:\Users\14128\.codex")
target_id = "019dcdb0-045f-7f42-9186-97916c4ea00d"
project = r"D:\360Downloads\ziliao\ts\swf-eat-record"
project_lower = project[:1].lower() + project[1:]
project_sqlite = r"\\?\D:\360Downloads\ziliao\ts\swf-eat-record"
provider = "aimai1"
now = int(time.time())

state_path = codex / ".codex-global-state.json"
state_bak_path = codex / ".codex-global-state.json.bak"
index_path = codex / "session_index.jsonl"
db_path = codex / "state_5.sqlite"
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-single-restore-019dcdb0-swf-{now}"


def iso(sec):
    return datetime.fromtimestamp(sec, tz=timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def ids_from_order(value):
    if isinstance(value, dict) and isinstance(value.get("threadIds"), list):
        return [item for item in value["threadIds"] if isinstance(item, str)]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, str)]
    return []


def unique(values):
    seen = set()
    result = []
    for value in values:
        if value and value not in seen:
            seen.add(value)
            result.append(value)
    return result


def thread_order(ids):
    return {"threadIds": unique(ids)}


def sqlite_path(path_text):
    path_text = str(path_text or "").replace("/", "\\")
    if path_text.startswith("\\\\?\\"):
        return path_text
    if len(path_text) >= 3 and path_text[1] == ":":
        return "\\\\?\\" + path_text
    return path_text


def fs_path(path_text):
    path_text = str(path_text or "")
    if path_text.startswith("\\\\?\\"):
        return Path(path_text[4:])
    return Path(path_text)


backup_dir.mkdir(parents=True, exist_ok=True)
for path in [state_path, state_bak_path, index_path, db_path]:
    if path.exists():
        shutil.copy2(path, backup_dir / path.name)

con = sqlite3.connect(db_path)
con.row_factory = sqlite3.Row
row = con.execute("select * from threads where id=?", (target_id,)).fetchone()
if row is None:
    raise SystemExit(f"sqlite thread not found: {target_id}")

rollout = fs_path(row["rollout_path"])
if not rollout.exists():
    matches = list((codex / "sessions").rglob(f"*{target_id}.jsonl"))
    if matches:
        rollout = matches[0]
if rollout.exists():
    shutil.copy2(rollout, backup_dir / rollout.name)

rollout_sqlite = sqlite_path(str(rollout)) if rollout.exists() else sqlite_path(row["rollout_path"])
con.execute(
    "update threads set cwd=?, model_provider=?, archived=0, archived_at=NULL, rollout_path=?, updated_at=?, updated_at_ms=? where id=?",
    (project_sqlite, provider, rollout_sqlite, now, now * 1000, target_id),
)
con.execute(
    "update thread_dynamic_tools set namespace='codex_app' where thread_id=? and namespace is null",
    (target_id,),
)
con.commit()
con.close()

changed_meta = 0
if rollout.exists():
    text = rollout.read_text(encoding="utf-8", errors="ignore")
    line_ending = "\r\n" if "\r\n" in text else "\n"
    next_lines = []
    for line in text.splitlines():
        try:
            item = json.loads(line)
        except Exception:
            next_lines.append(line)
            continue
        if item.get("type") == "session_meta" and isinstance(item.get("payload"), dict):
            payload = item["payload"]
            before = (payload.get("id"), payload.get("cwd"), payload.get("model_provider"))
            payload["id"] = target_id
            payload["cwd"] = project
            payload["model_provider"] = provider
            if before != (payload.get("id"), payload.get("cwd"), payload.get("model_provider")):
                changed_meta += 1
        next_lines.append(json.dumps(item, ensure_ascii=False, separators=(",", ":")))
    rollout.write_text(line_ending.join(next_lines) + line_ending, encoding="utf-8")

state = json.loads(state_path.read_text(encoding="utf-8", errors="ignore")) if state_path.exists() else {}
if not isinstance(state, dict):
    state = {}

for key in ["active-workspace-roots", "electron-saved-workspace-roots", "project-order"]:
    values = state.get(key)
    if not isinstance(values, list):
        values = []
    values = [item for item in values if isinstance(item, str) and item not in [project, project_lower, project_sqlite]]
    state[key] = unique([project] + values)

orders = state.setdefault("sidebar-project-thread-orders", {})
if not isinstance(orders, dict):
    orders = {}
    state["sidebar-project-thread-orders"] = orders
normal_ids = ids_from_order(orders.pop(project, None))
lower_ids = ids_from_order(orders.pop(project_lower, None))
sqlite_ids = ids_from_order(orders.pop(project_sqlite, None))
for key, value in list(orders.items()):
    ids = [item for item in ids_from_order(value) if item != target_id]
    orders[key] = thread_order(ids)
orders[project] = thread_order([target_id] + normal_ids + lower_ids + sqlite_ids)

chat_order = ids_from_order(state.get("sidebar-chat-thread-order"))
state["sidebar-chat-thread-order"] = thread_order([target_id] + [item for item in chat_order if item != target_id])

projectless = state.get("projectless-thread-ids")
if isinstance(projectless, list):
    state["projectless-thread-ids"] = [item for item in projectless if item != target_id]

hints = state.setdefault("thread-workspace-root-hints", {})
if isinstance(hints, dict):
    hints[target_id] = project

prompt = row["title"] or row["first_user_message"] or target_id
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
    atom["sidebar-keep-projects-in-recent-v1"] = True
    atom["projectless-sidebar-chats-first-v1"] = False
    collapsed = atom.setdefault("sidebar-collapsed-groups", {})
    if isinstance(collapsed, dict):
        collapsed.pop(project, None)
        collapsed.pop(project_lower, None)
        collapsed.pop(project_sqlite, None)
    sections = atom.setdefault("sidebar-collapsed-sections-v1", {})
    if isinstance(sections, dict):
        sections["chats"] = False
        sections["pinned"] = False
        sections["threads"] = False
    prompt_history = atom.setdefault("prompt-history", {})
    if isinstance(prompt_history, dict):
        prompt_history[target_id] = [prompt]

state_text = json.dumps(state, ensure_ascii=False, indent=2) + "\n"
state_path.write_text(state_text, encoding="utf-8")
if state_bak_path.exists():
    state_bak_path.write_text(state_text, encoding="utf-8")

records = []
if index_path.exists():
    for line in index_path.read_text(encoding="utf-8", errors="ignore").splitlines():
        try:
            item = json.loads(line)
        except Exception:
            continue
        if item.get("id") != target_id:
            records.append(item)
front_item = {"id": target_id, "thread_name": row["title"] or row["first_user_message"] or target_id, "updated_at": iso(now)}
index_path.write_text("\n".join(json.dumps(item, ensure_ascii=False, separators=(",", ":")) for item in [front_item] + records) + "\n", encoding="utf-8")

print(f"backup={backup_dir}")
print(f"restored={target_id}")
print(f"project={project}")
print(f"rollout={rollout}")
print(f"rollout_sqlite={rollout_sqlite}")
print(f"rollout_meta_changed={changed_meta}")
