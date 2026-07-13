import sqlite3
from pathlib import Path
for dbname in ['logs_2.sqlite','state_5.sqlite']:
    db=Path(r'C:\Users\14128\.codex')/dbname
    con=sqlite3.connect(db)
    print('\nDB', dbname)
    print(con.execute("select name,type from sqlite_master where type in ('table','index') order by type,name").fetchall())
    for (name,) in con.execute("select name from sqlite_master where type='table'"):
        print('TABLE', name, con.execute(f'pragma table_info({name})').fetchall())
