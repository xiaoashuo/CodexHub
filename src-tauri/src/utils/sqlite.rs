use crate::constants::AUDIT_DATABASE_RELATIVE_PATH;
use crate::utils::file::{build_user_relative_path, ensure_parent_dir};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection};
use serde_json::Value;
use std::collections::HashMap;

const AUDIT_SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    created_at TEXT NOT NULL,
    source_ip TEXT NOT NULL DEFAULT '',
    method TEXT NOT NULL DEFAULT '',
    path TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT '',
    target_provider TEXT NOT NULL DEFAULT '',
    cost TEXT NOT NULL DEFAULT '',
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cached_input_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    usage_source TEXT NOT NULL DEFAULT '',
    error_detail TEXT NOT NULL DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at);
CREATE INDEX IF NOT EXISTS idx_audit_logs_provider ON audit_logs(target_provider);
"#;

pub(crate) fn audit_database_path() -> Result<std::path::PathBuf, String> {
    build_user_relative_path(AUDIT_DATABASE_RELATIVE_PATH)
}

pub(crate) fn open_sqlite_database() -> Result<Connection, String> {
    let path = audit_database_path()?;
    ensure_parent_dir(&path)?;
    let connection = Connection::open(&path)
        .map_err(|error| format!("打开 SQLite 数据库失败：{}，路径：{}", error, path.display()))?;
    connection.execute_batch("PRAGMA busy_timeout = 5000;")
        .map_err(|error| format!("设置 SQLite 数据库参数失败：{}", error))?;
    connection.execute_batch(AUDIT_SCHEMA)
        .map_err(|error| format!("初始化 SQLite 数据库表失败：{}", error))?;
    Ok(connection)
}

pub(crate) fn initialize_sqlite_database() -> Result<(), String> {
    let connection = open_sqlite_database()?;
    connection.execute_batch(AUDIT_SCHEMA)
        .map_err(|error| format!("初始化 SQLite 数据库表失败：{}", error))
}

pub(crate) fn insert_audit_log(
    created_at: &str,
    source_ip: &str,
    method: &str,
    path: &str,
    status: &str,
    target_provider: &str,
    cost: &str,
    input_tokens: u64,
    output_tokens: u64,
    cached_input_tokens: u64,
    total_tokens: u64,
    usage_source: &str,
    error_detail: &str,
) -> Result<usize, String> {
    let connection = open_sqlite_database()?;
    connection.execute_batch(AUDIT_SCHEMA)
        .map_err(|error| format!("初始化审计日志表失败：{}", error))?;
    connection.execute(
        "INSERT INTO audit_logs (created_at, source_ip, method, path, status, target_provider, cost, input_tokens, output_tokens, cached_input_tokens, total_tokens, usage_source, error_detail) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![created_at, source_ip, method, path, status, target_provider, cost, input_tokens, output_tokens, cached_input_tokens, total_tokens, usage_source, error_detail],
    ).map_err(|error| format!("写入审计日志失败：{}", error))
}

pub(crate) fn sqlite_execute(sql: &str, params: &[SqlValue]) -> Result<usize, String> {
    let connection = open_sqlite_database()?;
    connection.execute(sql, params_from_iter(params.iter()))
        .map_err(|error| format!("执行 SQLite 写操作失败：{}", error))
}

pub(crate) fn sqlite_query(sql: &str, params: &[SqlValue]) -> Result<Vec<HashMap<String, Value>>, String> {
    let connection = open_sqlite_database()?;
    let mut statement = connection.prepare(sql)
        .map_err(|error| format!("准备 SQLite 查询失败：{}", error))?;
    let column_names = statement.column_names().iter().map(|name| name.to_string()).collect::<Vec<_>>();
    let rows = statement.query_map(params_from_iter(params.iter()), |row| {
        let mut item = HashMap::new();
        for (index, name) in column_names.iter().enumerate() {
            let value = sqlite_value_to_json(row.get_ref(index)?)?;
            item.insert(name.clone(), value);
        }
        Ok(item)
    }).map_err(|error| format!("执行 SQLite 查询失败：{}", error))?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|error| format!("读取 SQLite 查询结果失败：{}", error))
}

pub(crate) fn sqlite_insert(table: &str, columns: &[&str], values: &[SqlValue]) -> Result<usize, String> {
    if columns.is_empty() || columns.len() != values.len() || !is_safe_identifier(table) || columns.iter().any(|column| !is_safe_identifier(column)) {
        return Err("SQLite 插入参数无效".to_string());
    }
    let columns_text = columns.join(", ");
    let placeholders = std::iter::repeat("?").take(values.len()).collect::<Vec<_>>().join(", ");
    sqlite_execute(&format!("INSERT INTO {} ({}) VALUES ({})", table, columns_text, placeholders), values)
}

pub(crate) fn sqlite_update(table: &str, assignments: &[&str], where_clause: &str, params: &[SqlValue]) -> Result<usize, String> {
    if assignments.is_empty() || !is_safe_identifier(table) || assignments.iter().any(|assignment| !is_safe_identifier(assignment)) {
        return Err("SQLite 更新参数无效".to_string());
    }
    sqlite_execute(&format!("UPDATE {} SET {} WHERE {}", table, assignments.iter().map(|column| format!("{} = ?", column)).collect::<Vec<_>>().join(", "), where_clause), params)
}

pub(crate) fn sqlite_delete(table: &str, where_clause: &str, params: &[SqlValue]) -> Result<usize, String> {
    if !is_safe_identifier(table) {
        return Err("SQLite 删除参数无效".to_string());
    }
    sqlite_execute(&format!("DELETE FROM {} WHERE {}", table, where_clause), params)
}

fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn sqlite_value_to_json(value: rusqlite::types::ValueRef<'_>) -> rusqlite::Result<Value> {
    Ok(match value {
        rusqlite::types::ValueRef::Null => Value::Null,
        rusqlite::types::ValueRef::Integer(number) => Value::Number(number.into()),
        rusqlite::types::ValueRef::Real(number) => serde_json::Number::from_f64(number).map(Value::Number).unwrap_or(Value::Null),
        rusqlite::types::ValueRef::Text(text) => Value::String(String::from_utf8_lossy(text).to_string()),
        rusqlite::types::ValueRef::Blob(bytes) => Value::String(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes)),
    })
}
