use crate::*;
use crate::constants::*;

pub(crate) fn json_response(
    status_code: u16,
    body: String,
    target_provider: impl Into<String>,
    error_detail: impl Into<String>,
) -> RouterResponse {
    RouterResponse {
        flush_headers_before_body: false,
        status_code,
        content_type: HEADER_JSON.to_string(),
        body,
        target_provider: target_provider.into(),
        error_detail: error_detail.into(),
        usage: None,
        usage_source: TOKEN_USAGE_SOURCE_MISSING.to_string(),
    }
}

pub(crate) fn html_response(
    status_code: u16,
    body: String,
    target_provider: impl Into<String>,
    error_detail: impl Into<String>,
) -> RouterResponse {
    RouterResponse {
        flush_headers_before_body: false,
        status_code,
        content_type: "text/html; charset=utf-8".to_string(),
        body: format!(
            "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>Codex OAuth</title><style>body{{font-family:system-ui,-apple-system,BlinkMacSystemFont,\"Segoe UI\",sans-serif;margin:48px;color:#0f172a}}p{{color:#475569}}</style></head><body>{}</body></html>",
            body
        ),
        target_provider: target_provider.into(),
        error_detail: error_detail.into(),
        usage: None,
        usage_source: TOKEN_USAGE_SOURCE_MISSING.to_string(),
    }
}
