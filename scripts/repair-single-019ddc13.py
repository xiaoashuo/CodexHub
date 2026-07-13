import json
import shutil
import sqlite3
import time
from datetime import datetime, timezone
from pathlib import Path

codex = Path(r"C:\Users\14128\.codex")
target_id = "019ddc13-84f6-7843-b215-af1ea9c4b41e"
target_cwd = r"D:\360Downloads\ziliao\ts\swf-eat-record"
target_cwd_sqlite = r"\\?\D:\360Downloads\ziliao\ts\swf-eat-record"
target_provider = "aimai1"
state_path = codex / ".codex-global-state.json"
index_path = codex / "session_index.jsonl"
db_path = codex / "state_5.sqlite"
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-single-restore-019ddc13-{int(time.time())}"
now = int(time.time())

def iso(sec):
    return datetime.fromtimestamp(int(sec), tz=timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")

def ids_from(v):
    if isinstance(v, dict) and isinstance(v.get("threadIds"), list):
        return [x for x in v["threadIds"] if isinstance(x, str)]
    if isinstance(v, list):
        return [x for x in v if isinstance(x, str)]
    return []

def unique(values):
    seen = set(); out = []
    for v in values:
        if v and v not in seen:
            seen.add(v); out.append(v)
    return out

def front(current):
    return unique([target_id] + [x for x in current if x != target_id])

def order(ids):
    return {"threadIds": unique(ids)}

backup_dir.mkdir(parents=True, exist_ok=True)
for path in [state_path, index_path, db_path]:
    if path.exists():
        shutil.copy2(path, backup_dir / path.name)

con = sqlite3.connect(db_path)
con.row_factory = sqlite3.Row
row = con.execute("select * from threads where id=?", (target_id,)).fetchone()
if not row:
    raise SystemExit(f"sqlite thread not found: {target_id}")
rollout = Path(row["rollout_path"])
if rollout.exists():
    shutil.copy2(rollout, backup_dir / rollout.name)

# 1) sqlite 刷新为最近，保持 aimai1 和 swf cwd。
con.execute(
    "update threads set updated_at=?, updated_at_ms=?, model_provider=?, cwd=?, archived=0 where id=?",
    (now, now * 1000, target_provider, target_cwd_sqlite, target_id),
)
con.execute(
    "update thread_dynamic_tools set namespace='codex_app' where thread_id=? and namespace is null",
    (target_id,),
)
con.commit()
con.close()

# 2) rollout session_meta 再统一一次。
if rollout.exists():
    text = rollout.read_text(encoding="utf-8", errors="ignore")
    line_ending = "\r\n" if "\r\n" in text else "\n"
    out = []
    changed = 0
    for line in text.splitlines():
        try:
            item = json.loads(line)
        except Exception:
            out.append(line)
            continue
        if item.get("type") == "session_meta" and isinstance(item.get("payload"), dict):
            payload = item["payload"]
            old = (payload.get("id"), payload.get("cwd"), payload.get("model_provider"))
            payload["id"] = target_id
            payload["cwd"] = target_cwd
            payload["model_provider"] = target_provider
            if old != (payload.get("id"), payload.get("cwd"), payload.get("model_provider")):
                changed += 1
        out.append(json.dumps(item, ensure_ascii=False, separators=(",", ":")))
    rollout.write_text(line_ending.join(out) + line_ending, encoding="utf-8")
else:
    changed = 0

# 3) global-state 放到项目和 chat 第一位。
state = json.loads(state_path.read_text(encoding="utf-8", errors="ignore"))
if not isinstance(state, dict):
    state = {}
for key in ["active-workspace-roots", "electron-saved-workspace-roots", "project-order"]:
    vals = state.get(key)
    if not isinstance(vals, list):
        vals = []
    vals = [v[4:] if isinstance(v, str) and v.startswith("\\\\?") else v for v in vals if isinstance(v, str)]
    state[key] = unique([target_cwd] + vals)
orders = state.setdefault("sidebar-project-thread-orders", {})
if not isinstance(orders, dict):
    orders = {}; state["sidebar-project-thread-orders"] = orders
orders[target_cwd] = order(front(ids_from(orders.get(target_cwd)) + ids_from(orders.pop(target_cwd_sqlite, None))))
state["sidebar-chat-thread-order"] = order(front(ids_from(state.get("sidebar-chat-thread-order"))))
hints = state.setdefault("thread-workspace-root-hints", {})
if isinstance(hints, dict):
    hints[target_id] = target_cwd
for atom_key in ["persisted-atom-state", "electron-persisted-atom-state"]:
    atom = state.setdefault(atom_key, {})
    if not isinstance(atom, dict):
        atom = {}; state[atom_key] = atom
    atom["sidebar-workspace-filter-v2"] = "all"
    atom["sidebar-organize-mode-v1"] = "project"
    atom["sidebar-history-show"] = "all"
    atom["sidebar-history-organize"] = "project"
    atom["thread-sort-key"] = "updated_at"
    sections = atom.setdefault("sidebar-collapsed-sections-v1", {})
    if isinstance(sections, dict):
        sections["chats"] = False; sections["pinned"] = False; sections["threads"] = False
    collapsed = atom.setdefault("sidebar-collapsed-groups", {})
    if isinstance(collapsed, dict):
        collapsed.pop(target_cwd, None); collapsed.pop(target_cwd_sqlite, None); collapsed.pop(target_cwd.lower(), None)
    ph = atom.setdefault("prompt-history", {})
    if isinstance(ph, dict):
        ph[target_id] = [row["first_user_message"] or row["title"] or target_id]
state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

# 4) session_index 放第一行。
records = []
if index_path.exists():
    for line in index_path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            item = json.loads(line)
        except Exception:
            continue
        if item.get("id") != target_id:
            records.append(item)
front_item = {"id": target_id, "thread_name": row["title"] or row["first_user_message"] or target_id, "updated_at": iso(now)}
index_path.write_text("\n".join(json.dumps(x, ensure_ascii=False, separators=(",", ":")) for x in [front_item] + records) + "\n", encoding="utf-8")

print(f"backup={backup_dir}")
print(f"restored={target_id}")
print(f"rollout_meta_changed={changed}")
print(f"updated_at={now} {iso(now)}")
