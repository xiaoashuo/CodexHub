import json
import shutil
import sqlite3
import time
from datetime import datetime, timezone
from pathlib import Path

codex = Path(r"C:\Users\14128\.codex")
target_ids = [
    "019e15de-9c87-7531-a6c4-ddd7b284982f",
    "019e15a9-9374-77a1-a7c8-98d1a9353c20",
    "019e158c-6f6f-7710-be4c-79299af70601",
    "019e01ab-4189-7db0-854d-036f41bc7eb6",
]
target_cwd = r"D:\360Downloads\ziliao\ts\swf-eat-record"
target_cwd_sqlite = r"\\?\D:\360Downloads\ziliao\ts\swf-eat-record"
target_provider = "aimai1"
state_path = codex / ".codex-global-state.json"
index_path = codex / "session_index.jsonl"
db_path = codex / "state_5.sqlite"
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-batch-restore-swf-{int(time.time())}"
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
    return unique(target_ids + [x for x in current if x not in target_ids])

def order(ids):
    return {"threadIds": unique(ids)}

def normalize_root(v):
    if isinstance(v, str) and v.startswith("\\\\?"):
        return v[4:]
    return v

backup_dir.mkdir(parents=True, exist_ok=True)
for path in [state_path, index_path, db_path]:
    if path.exists():
        shutil.copy2(path, backup_dir / path.name)

con = sqlite3.connect(db_path)
con.row_factory = sqlite3.Row
rows = []
missing = []
for tid in target_ids:
    row = con.execute("select * from threads where id=?", (tid,)).fetchone()
    if not row:
        missing.append(tid)
    else:
        rows.append(row)
        rollout = Path(row["rollout_path"])
        if rollout.exists():
            shutil.copy2(rollout, backup_dir / rollout.name)
if missing:
    raise SystemExit("sqlite thread not found: " + ",".join(missing))

# 1) sqlite：按传入顺序刷新为最近时间，间隔 60 秒，确保能进入 Codex 最近加载窗口。
for idx, row in enumerate(rows):
    ts = now - idx * 60
    con.execute(
        "update threads set updated_at=?, updated_at_ms=?, model_provider=?, cwd=?, archived=0 where id=?",
        (ts, ts * 1000, target_provider, target_cwd_sqlite, row["id"]),
    )
    con.execute(
        "update thread_dynamic_tools set namespace='codex_app' where thread_id=? and namespace is null",
        (row["id"],),
    )
con.commit()
con.close()

# 2) rollout：统一 session_meta.id/cwd/model_provider。
rollout_changes = {}
for row in rows:
    rollout = Path(row["rollout_path"])
    changed = 0
    meta_count = 0
    before_ids = {}
    before_providers = {}
    if rollout.exists():
        text = rollout.read_text(encoding="utf-8", errors="ignore")
        line_ending = "\r\n" if "\r\n" in text else "\n"
        out = []
        for line in text.splitlines():
            try:
                item = json.loads(line)
            except Exception:
                out.append(line)
                continue
            if item.get("type") == "session_meta" and isinstance(item.get("payload"), dict):
                meta_count += 1
                payload = item["payload"]
                before_ids[payload.get("id")] = before_ids.get(payload.get("id"), 0) + 1
                before_providers[payload.get("model_provider")] = before_providers.get(payload.get("model_provider"), 0) + 1
                old = (payload.get("id"), payload.get("cwd"), payload.get("model_provider"))
                payload["id"] = row["id"]
                payload["cwd"] = target_cwd
                payload["model_provider"] = target_provider
                if old != (payload.get("id"), payload.get("cwd"), payload.get("model_provider")):
                    changed += 1
            out.append(json.dumps(item, ensure_ascii=False, separators=(",", ":")))
        rollout.write_text(line_ending.join(out) + line_ending, encoding="utf-8")
    rollout_changes[row["id"]] = {"meta_count": meta_count, "changed": changed, "before_ids": before_ids, "before_providers": before_providers}

# 3) global-state：项目 roots、项目线程顺序、chat 顺序、hint、折叠状态、prompt-history。
state = json.loads(state_path.read_text(encoding="utf-8", errors="ignore"))
if not isinstance(state, dict):
    state = {}
for key in ["active-workspace-roots", "electron-saved-workspace-roots", "project-order"]:
    vals = state.get(key)
    if not isinstance(vals, list):
        vals = []
    vals = [normalize_root(v) for v in vals if isinstance(v, str)]
    state[key] = unique([target_cwd] + vals)
orders = state.setdefault("sidebar-project-thread-orders", {})
if not isinstance(orders, dict):
    orders = {}; state["sidebar-project-thread-orders"] = orders
orders[target_cwd] = order(front(ids_from(orders.get(target_cwd)) + ids_from(orders.pop(target_cwd_sqlite, None))))
state["sidebar-chat-thread-order"] = order(front(ids_from(state.get("sidebar-chat-thread-order"))))
hints = state.setdefault("thread-workspace-root-hints", {})
if not isinstance(hints, dict):
    hints = {}; state["thread-workspace-root-hints"] = hints
for tid in target_ids:
    hints[tid] = target_cwd
for atom_key in ["persisted-atom-state", "electron-persisted-atom-state"]:
    atom = state.setdefault(atom_key, {})
    if not isinstance(atom, dict):
        atom = {}; state[atom_key] = atom
    atom["sidebar-workspace-filter-v2"] = "all"
    atom["sidebar-organize-mode-v1"] = "project"
    atom["sidebar-history-show"] = "all"
    atom["sidebar-history-organize"] = "project"
    atom["thread-sort-key"] = "updated_at"
    atom["sidebar-move-updated-threads-in-front-v1"] = True
    atom["projectless-sidebar-chats-first-v1"] = False
    sections = atom.setdefault("sidebar-collapsed-sections-v1", {})
    if isinstance(sections, dict):
        sections["chats"] = False; sections["pinned"] = False; sections["threads"] = False
    collapsed = atom.setdefault("sidebar-collapsed-groups", {})
    if isinstance(collapsed, dict):
        collapsed.pop(target_cwd, None); collapsed.pop(target_cwd_sqlite, None); collapsed.pop(target_cwd.lower(), None)
    ph = atom.setdefault("prompt-history", {})
    if isinstance(ph, dict):
        for row in rows:
            ph[row["id"]] = [row["first_user_message"] or row["title"] or row["id"]]
state_path.write_text(json.dumps(state, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

# 4) session_index：批量放前 4 行。
records = []
if index_path.exists():
    for line in index_path.read_text(encoding="utf-8", errors="ignore").splitlines():
        if not line.strip():
            continue
        try:
            item = json.loads(line)
        except Exception:
            continue
        if item.get("id") not in target_ids:
            records.append(item)
front_items = []
for idx, row in enumerate(rows):
    ts = now - idx * 60
    front_items.append({"id": row["id"], "thread_name": row["title"] or row["first_user_message"] or row["id"], "updated_at": iso(ts)})
index_path.write_text("\n".join(json.dumps(x, ensure_ascii=False, separators=(",", ":")) for x in front_items + records) + "\n", encoding="utf-8")

print(f"backup={backup_dir}")
print(f"restored_count={len(rows)}")
for row in rows:
    info = rollout_changes[row["id"]]
    print(f"{row['id']} meta_count={info['meta_count']} changed={info['changed']} before_ids={info['before_ids']} before_providers={info['before_providers']}")
print("order=" + ",".join(target_ids))
