import json
import shutil
import time
from pathlib import Path

codex = Path(r"C:\Users\14128\.codex")
target_id = "019e15de-9c87-7531-a6c4-ddd7b284982f"
target_cwd = r"D:\360Downloads\ziliao\ts\swf-eat-record"
target_provider = "aimai1"
rollout = codex / "sessions" / "2026" / "05" / "11" / "rollout-2026-05-11T15-09-31-019e15de-9c87-7531-a6c4-ddd7b284982f.jsonl"
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-rollout-meta-repair-019e15de-{int(time.time())}"

if not rollout.exists():
    raise SystemExit(f"rollout not found: {rollout}")
backup_dir.mkdir(parents=True, exist_ok=True)
backup_path = backup_dir / rollout.name
shutil.copy2(rollout, backup_path)

text = rollout.read_text(encoding="utf-8", errors="ignore")
line_ending = "\r\n" if "\r\n" in text else "\n"
changed = 0
meta_count = 0
before_ids = {}
before_cwds = {}
before_providers = {}
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
        before_cwds[payload.get("cwd")] = before_cwds.get(payload.get("cwd"), 0) + 1
        before_providers[payload.get("model_provider")] = before_providers.get(payload.get("model_provider"), 0) + 1
        old = (payload.get("id"), payload.get("cwd"), payload.get("model_provider"))
        payload["id"] = target_id
        payload["cwd"] = target_cwd
        payload["model_provider"] = target_provider
        if old != (payload.get("id"), payload.get("cwd"), payload.get("model_provider")):
            changed += 1

    out.append(json.dumps(item, ensure_ascii=False, separators=(",", ":")))

rollout.write_text(line_ending.join(out) + line_ending, encoding="utf-8")

print(f"backup={backup_path}")
print(f"rollout={rollout}")
print(f"session_meta_count={meta_count}")
print(f"changed_session_meta={changed}")
print(f"before_ids={before_ids}")
print(f"before_cwds={before_cwds}")
print(f"before_providers={before_providers}")
