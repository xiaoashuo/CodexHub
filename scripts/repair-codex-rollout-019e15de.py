import json
import shutil
from pathlib import Path

path = Path(r"C:\Users\14128\.codex\sessions\2026\05\11\rollout-2026-05-11T15-09-31-019e15de-9c87-7531-a6c4-ddd7b284982f.jsonl")
target_id = "019e15de-9c87-7531-a6c4-ddd7b284982f"
target_cwd = r"D:\360Downloads\ziliao\ts\swf-eat-record"
target_provider = "aimai1"

if not path.exists():
    raise SystemExit(f"file not found: {path}")

backup = path.with_suffix(path.suffix + ".before-id-repair.bak")
shutil.copy2(path, backup)
text = path.read_text(encoding="utf-8", errors="ignore")
line_ending = "\r\n" if "\r\n" in text else "\n"
changed = 0
out = []

for line in text.splitlines():
    try:
        item = json.loads(line)
    except Exception:
        out.append(line)
        continue

    if item.get("type") == "session_meta" and isinstance(item.get("payload"), dict):
        payload = item["payload"]
        before = (payload.get("id"), payload.get("cwd"), payload.get("model_provider"))
        payload["id"] = target_id
        payload["cwd"] = target_cwd
        payload["model_provider"] = target_provider
        after = (payload.get("id"), payload.get("cwd"), payload.get("model_provider"))
        if before != after:
            changed += 1

    out.append(json.dumps(item, ensure_ascii=False, separators=(",", ":")))

path.write_text(line_ending.join(out) + line_ending, encoding="utf-8")
print(f"backup={backup}")
print(f"changed_session_meta={changed}")
