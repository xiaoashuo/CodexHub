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
backup_dir = codex / "ai-router-workspace" / "backup" / "sessions" / f"manual-swf-sidebar-force-recent-{int(time.time())}"
project = r"D:\360Downloads\ziliao\ts\swf-eat-record"
project_ext = r"\\?\D:\360Downloads\ziliao\ts\swf-eat-record"
now = int(time.time())

def normalize_path(v):
    v=(v or '').strip()
    if v.startswith("\\?"):
        return v[4:]
    return v

def unique(items):
    seen=set(); out=[]
    for x in items:
        if x and x not in seen:
            seen.add(x); out.append(x)
    return out

def ids_from(v):
    if isinstance(v,dict) and isinstance(v.get('threadIds'),list): return [x for x in v['threadIds'] if isinstance(x,str)]
    if isinstance(v,list): return [x for x in v if isinstance(x,str)]
    return []

def order(ids): return {'threadIds': unique(ids)}
def front(cur, sel): return unique(list(sel)+[x for x in cur if x not in sel])
def iso(sec): return datetime.fromtimestamp(sec, tz=timezone.utc).replace(microsecond=0).isoformat().replace('+00:00','Z')

backup_dir.mkdir(parents=True, exist_ok=True)
for p in [db_path,state_path,index_path]:
    if p.exists(): shutil.copy2(p, backup_dir / p.name)

con=sqlite3.connect(db_path); con.row_factory=sqlite3.Row
rows=con.execute("""
SELECT id, COALESCE(NULLIF(title,''), NULLIF(first_user_message,''), id) title,
       COALESCE(NULLIF(first_user_message,''), NULLIF(title,''), id) prompt
FROM threads
WHERE COALESCE(archived,0)=0 AND replace(cwd,'\\\\?\\','')=?
ORDER BY COALESCE(updated_at_ms, updated_at*1000) DESC, id ASC
""", (project,)).fetchall()
ids=[r['id'] for r in rows]
if not ids: raise SystemExit('no swf rows')
# 强制更新为最近，但保留相对顺序：第一条最新，后面每条差 60 秒。
for i, tid in enumerate(ids):
    ts=now - i*60
    con.execute("update threads set updated_at=?, updated_at_ms=?, cwd=?, model_provider='aimai1', archived=0 where id=?", (ts, ts*1000, project_ext, tid))
con.execute("UPDATE thread_dynamic_tools SET namespace='codex_app' WHERE thread_id IN (%s) AND namespace IS NULL" % ','.join(['?']*len(ids)), ids)
con.commit(); con.close()

state=json.loads(state_path.read_text(encoding='utf-8',errors='ignore'))
if not isinstance(state,dict): state={}
for key in ['active-workspace-roots','electron-saved-workspace-roots','project-order']:
    cur=state.get(key)
    if not isinstance(cur,list): cur=[]
    state[key]=front(unique([normalize_path(x) for x in cur if isinstance(x,str)]), [project])
orders=state.setdefault('sidebar-project-thread-orders',{})
if not isinstance(orders,dict): orders={}; state['sidebar-project-thread-orders']=orders
orders[project]=order(front(ids_from(orders.get(project)) + ids_from(orders.pop(project_ext, None)), ids))
state['sidebar-chat-thread-order']=order(front(ids_from(state.get('sidebar-chat-thread-order')), ids))
hints=state.setdefault('thread-workspace-root-hints',{})
if not isinstance(hints,dict): hints={}; state['thread-workspace-root-hints']=hints
for tid in ids: hints[tid]=project
for ak in ['persisted-atom-state','electron-persisted-atom-state']:
    atom=state.setdefault(ak,{})
    if not isinstance(atom,dict): atom={}; state[ak]=atom
    atom['sidebar-workspace-filter-v2']='all'
    atom['sidebar-organize-mode-v1']='project'
    atom['sidebar-history-show']='all'
    atom['sidebar-history-organize']='project'
    atom['thread-sort-key']='updated_at'
    atom['sidebar-move-updated-threads-in-front-v1']=True
    atom['projectless-sidebar-chats-first-v1']=False
    cg=atom.setdefault('sidebar-collapsed-groups',{})
    if isinstance(cg,dict):
        cg.pop(project,None); cg.pop(project_ext,None); cg.pop(project.lower(),None)
    sec=atom.setdefault('sidebar-collapsed-sections-v1',{})
    if isinstance(sec,dict):
        sec['chats']=False; sec['pinned']=False; sec['threads']=False
    ph=atom.setdefault('prompt-history',{})
    if isinstance(ph,dict):
        for r in rows: ph[r['id']]=[r['prompt']]
state_path.write_text(json.dumps(state,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')

# session_index 前置并使用新的 updated_at。
existing=[]; seen=set(ids)
if index_path.exists():
    for line in index_path.read_text(encoding='utf-8',errors='ignore').splitlines():
        try: item=json.loads(line)
        except Exception: continue
        tid=item.get('id')
        if isinstance(tid,str) and tid and tid not in seen:
            seen.add(tid); existing.append(item)
front_items=[]
for i,r in enumerate(rows):
    ts=now - i*60
    front_items.append({'id':r['id'],'thread_name':r['title'],'updated_at':iso(ts)})
index_path.write_text('\n'.join(json.dumps(x,ensure_ascii=False,separators=(',',':')) for x in front_items+existing)+'\n',encoding='utf-8')
print('backup=', backup_dir)
print('updated_count=', len(ids))
print('first_ids=', ids[:5])
print('now=', now, iso(now))


