use crate::*;
use crate::constants::*;
use std::collections::HashMap;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

pub(crate) fn base64_url_json(value: &serde_json::Value) -> Option<String> {
    serde_json::to_vec(value)
        .ok()
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn build_status_line(status_code: u16) -> String {
    let reason = match status_code {
        HTTP_OK => "OK",
        HTTP_NO_CONTENT => "No Content",
        HTTP_BAD_REQUEST => "Bad Request",
        HTTP_UNAUTHORIZED => "Unauthorized",
        HTTP_FORBIDDEN => "Forbidden",
        HTTP_NOT_FOUND => "Not Found",
        HTTP_METHOD_NOT_ALLOWED => "Method Not Allowed",
        HTTP_PAYLOAD_TOO_LARGE => "Payload Too Large",
        HTTP_TOO_MANY_REQUESTS => "Too Many Requests",
        HTTP_BAD_GATEWAY => "Bad Gateway",
        HTTP_SERVICE_UNAVAILABLE => "Service Unavailable",
        _ => "Upstream Response",
    };

    format!("HTTP/1.1 {} {}", status_code, reason)
}

pub(crate) fn parse_request_line(request_line: &str) -> (String, String) {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("UNKNOWN").to_string();
    let path = parts.next().unwrap_or("/").to_string();

    (method, path)
}

pub(crate) fn request_path_without_query(path: &str) -> &str {
    path.split_once('?')
        .map(|(clean_path, _)| clean_path)
        .unwrap_or(path)
}

pub(crate) fn oauth_redirect_uri() -> String {
    let callback_port = CODEX_OAUTH_CALLBACK_LISTENER_PORT
        .get()
        .copied()
        .unwrap_or_else(configured_oauth_callback_port);
    format!("http://localhost:{}{}", callback_port, OAUTH_CALLBACK_PATH)
}

pub(crate) fn random_base64_url(byte_count: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; byte_count];
    getrandom::getrandom(&mut bytes)
        .map_err(|error| format!("生成 OAuth 随机数失败：{}", error))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

pub(crate) fn parse_query_params(query: &str) -> HashMap<String, String> {
    let mut params = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        params.insert(url_decode_component(key), url_decode_component(value));
    }
    params
}

pub(crate) fn url_encode_component(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            b' ' => encoded.push('+'),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}

pub(crate) fn url_decode_component(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let hex = &value[index + 1..index + 3];
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte);
                    index += 3;
                } else {
                    decoded.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).to_string()
}

pub(crate) fn decode_jwt_payload(token: &str) -> Option<serde_json::Value> {
    let payload = token.split('.').nth(1)?;
    let bytes = URL_SAFE_NO_PAD.decode(payload).ok()?;
    serde_json::from_slice::<serde_json::Value>(&bytes).ok()
}

