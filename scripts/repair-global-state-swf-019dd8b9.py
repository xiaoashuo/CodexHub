import json
import shutil
import time
from pathlib import Path

codex = Path(r"C:\Users\14128\.codex")
state_path = codex / ".codex-global-state.json"
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-global-state-repair-{int(time.time())}"
target_id = "019dd8b9-7510-7df2-9f7b-97f5fc34dcc0"
project = r"D:\360Downloads\ziliao\ts\swf-eat-record"
project_variant = r"\\?\D:\360Downloads\ziliao\ts\swf-eat-record"


def normalize_path(value: str) -> str:
    value = (value or "").strip()
    if value.startswith(r"\\?\\"):
        return value[4:]
    if value.startswith(r"\\?"):
        return value[4:]
    return value


def unique(values):
    seen = set()
    result = []
    for value in values:
        if value and value not in seen:
            seen.add(value)
            result.append(value)
    return result


def order_ids(value):
    if isinstance(value, dict) and isinstance(value.get("threadIds"), list):
        return [item for item in value["threadIds"] if isinstance(item, str)]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, str)]
    return []


def thread_order(ids):
    return {"threadIds": unique(ids)}


if not state_path.exists():
    raise SystemExit(f"state file not found: {state_path}")

backup_dir.mkdir(parents=True, exist_ok=True)
backup_path = backup_dir / "codex-global-state.before.json"
shutil.copy2(state_path, backup_path)

root = json.loads(state_path.read_text(encoding="utf-8", errors="ignore"))
if not isinstance(root, dict):
    root = {}

for key in ["active-workspace-roots", "electron-saved-workspace-roots", "project-order"]:
    values = root.get(key)
    if not isinstance(values, list):
        values = []
    normalized = unique([normalize_path(item) for item in values if isinstance(item, str)])
    root[key] = unique([project] + normalized)

orders = root.setdefault("sidebar-project-thread-orders", {})
if not isinstance(orders, dict):
    orders = {}
    root["sidebar-project-thread-orders"] = orders
normal_ids = order_ids(orders.get(project))
variant_ids = order_ids(orders.pop(project_variant, None))
orders[project] = thread_order([target_id] + normal_ids + variant_ids)

hints = root.setdefault("thread-workspace-root-hints", {})
if not isinstance(hints, dict):
    hints = {}
    root["thread-workspace-root-hints"] = hints
hints[target_id] = project

chat_order = order_ids(root.get("sidebar-chat-thread-order"))
root["sidebar-chat-thread-order"] = thread_order([target_id] + chat_order)

for atom_key in ["persisted-atom-state", "electron-persisted-atom-state"]:
    atom = root.setdefault(atom_key, {})
    if not isinstance(atom, dict):
        atom = {}
        root[atom_key] = atom
    atom["sidebar-workspace-filter-v2"] = "all"
    atom["sidebar-organize-mode-v1"] = "project"
    atom["sidebar-keep-projects-in-recent-v1"] = True
    atom["projectless-sidebar-chats-first-v1"] = False
    atom["thread-sort-key"] = "updated_at"
    atom["sidebar-move-updated-threads-in-front-v1"] = True
    atom["sidebar-history-show"] = "all"
    atom["sidebar-history-organize"] = "project"
    atom["organize-mode-v1"] = "project"
    collapsed = atom.setdefault("sidebar-collapsed-groups", {})
    if not isinstance(collapsed, dict):
        collapsed = {}
        atom["sidebar-collapsed-groups"] = collapsed
    collapsed.pop(project, None)
    collapsed.pop(project_variant, None)
    sections = atom.setdefault("sidebar-collapsed-sections-v1", {})
    if not isinstance(sections, dict):
        sections = {}
        atom["sidebar-collapsed-sections-v1"] = sections
    sections["chats"] = False
    sections["pinned"] = False
    sections["threads"] = False

state_path.write_text(json.dumps(root, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

print(f"backup={backup_path}")
print(f"repaired_project={project}")
print(f"target_id={target_id}")
print("active_roots=", [item for item in root.get("active-workspace-roots", []) if "swf-eat-record" in str(item)])
print("project_order=", [item for item in root.get("project-order", []) if "swf-eat-record" in str(item)])
print("electron_saved_roots=", [item for item in root.get("electron-saved-workspace-roots", []) if "swf-eat-record" in str(item)])
print("project_thread_order_has_target=", target_id in order_ids(root.get("sidebar-project-thread-orders", {}).get(project)))
print("chat_thread_order_has_target=", target_id in order_ids(root.get("sidebar-chat-thread-order")))
for atom_key in ["persisted-atom-state", "electron-persisted-atom-state"]:
    atom = root.get(atom_key, {})
    print(f"{atom_key}.sidebar-collapsed-groups.swf=", {
        key: value
        for key, value in (atom.get("sidebar-collapsed-groups") or {}).items()
        if "swf-eat-record" in str(key)
    })
    print(f"{atom_key}.sidebar-collapsed-sections-v1=", atom.get("sidebar-collapsed-sections-v1"))
