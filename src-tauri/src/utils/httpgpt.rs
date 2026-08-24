use std::path::Path;
use std::time::Duration;

use crate::accounts::read_accounts_registry;
use crate::constants::*;
use crate::{
    append_account_usage_router_log, append_internal_app_log, find_codex_access_token,
    find_codex_account_id, find_registry_account, find_string_by_keys, json_string_field,
    mask_secret, read_json_file_optional, should_log_verbose_account_usage_refresh, truncate_text,
    OfficialCodexForwardSettings,
};

pub(crate) fn active_account_codex_auth() -> Option<(String, String)> {
    let registry = read_accounts_registry().ok()?;
    let account_key = json_string_field(&registry, "activeAccountKey")
        .filter(|value| !value.trim().is_empty())?;
    let item = find_registry_account(&registry, &account_key)?;
    let snapshot_path = json_string_field(&item, "snapshotPath")?;
    let snapshot_root = read_json_file_optional(Path::new(&snapshot_path))?;
    let access_token = find_codex_access_token(&snapshot_root)
        .or_else(|| find_string_by_keys(&snapshot_root, &["OPENAI_API_KEY"]))?;
    let account_id = find_codex_account_id(&snapshot_root).unwrap_or(account_key);
    Some((access_token, account_id))
}

pub(crate) fn send_authenticated_chatgpt_backend_request(
    method: &str,
    url: &str,
    body: &[u8],
    access_token: &str,
    account_id: &str,
    proxy_url: Option<&str>,
) -> Result<(u16, String, String), ureq::Error> {
    let authorization = format!("Bearer {}", access_token);
    let agent = match proxy_url.and_then(|url| ureq::Proxy::new(url).ok()) {
        Some(proxy) => ureq::builder().proxy(proxy).build(),
        None => ureq::builder().build(),
    };
    let method_upper = method.to_uppercase();
    let mut request = match method_upper.as_str() {
        "GET" => agent.get(url),
        "POST" => agent.post(url),
        "PUT" => agent.put(url),
        "DELETE" => agent.delete(url),
        "PATCH" => agent.patch(url),
        "HEAD" => agent.head(url),
        "OPTIONS" => agent.request("OPTIONS", url),
        _ => agent.get(url),
    };
    request = request
        .timeout(Duration::from_secs(ACCOUNT_USAGE_REQUEST_TIMEOUT_SECONDS))
        .set(HEADER_ACCEPT, HEADER_JSON)
        .set(HEADER_CONTENT_TYPE, HEADER_JSON)
        .set(HEADER_AUTHORIZATION, &authorization)
        .set(HEADER_CHATGPT_ACCOUNT_ID, account_id)
        .set("Host", "chatgpt.com")
        .set(HEADER_OPENAI_BETA, OFFICIAL_CODEX_BETA_HEADER_VALUE)
        .set(HEADER_ORIGINATOR, OFFICIAL_CODEX_ORIGINATOR)
        .set(HEADER_ORIGIN, "https://chatgpt.com")
        .set(HEADER_REFERER, "https://chatgpt.com/")
        .set(HEADER_USER_AGENT, "Mozilla/5.0 codex-router-shell");

    let response = if matches!(method_upper.as_str(), "GET" | "HEAD" | "DELETE") {
        request.call()?
    } else {
        request.send_bytes(body)?
    };
    let status = response.status();
    let content_type = response.content_type().to_string();
    let body_string = response.into_string().unwrap_or_default();
    Ok((status, content_type, body_string))
}

pub(crate) fn fetch_codex_models(
    url: &str,
    access_token: &str,
    account_id: &str,
    proxy_url: Option<&str>,
) -> Result<String, String> {
    let response = send_authenticated_chatgpt_backend_request(
        "GET",
        url,
        &[],
        access_token,
        account_id,
        proxy_url,
    )
    .map_err(|error| {
        append_account_usage_router_log(url, account_id, "error", Some(&error.to_string()), None);
        format!("同步官方模型失败：{}", error)
    })?;
    let (status, _, body) = response;
    append_account_usage_router_log(url, account_id, &status.to_string(), None, Some(&body));
    if !(200..300).contains(&status) {
        return Err(format!("同步官方模型失败，HTTP 状态码：{}", status));
    }
    Ok(body)
}

pub(crate) fn send_codex_usage_request(
    url: &str,
    authorization: &str,
    account_id: &str,
    settings: &OfficialCodexForwardSettings,
    manual: bool,
    collect_error: &mut Option<String>,
) -> Option<String> {
    let access_token = authorization
        .strip_prefix("Bearer ")
        .unwrap_or(authorization);
    let response = send_authenticated_chatgpt_backend_request(
        "GET",
        url,
        &[],
        access_token,
        account_id,
        settings.proxy_url.as_deref(),
    );

    if should_log_verbose_account_usage_refresh(manual) {
        append_internal_app_log(
            "info",
            "accounts",
            "refresh-usage",
            "发送额度刷新请求",
            Some(format!(
                "method=GET, url={}, account={}",
                url,
                mask_secret(account_id)
            )),
        );
    }

    let (status, _, body) = match response {
        Ok(response) => response,
        Err(error) => {
            append_account_usage_router_log(url, account_id, "error", Some(&error.to_string()), None);
            *collect_error = Some(format!(
                "method=GET, url={}, account={}, error={}",
                url,
                mask_secret(account_id),
                error
            ));
            if should_log_verbose_account_usage_refresh(manual) {
                append_internal_app_log(
                    "warn",
                    "accounts",
                    "refresh-usage",
                    "请求额度接口失败",
                    Some(format!(
                        "method=GET, url={}, account={}, error={}",
                        url,
                        mask_secret(account_id),
                        error
                    )),
                );
            }
            return None;
        }
    };

    if should_log_verbose_account_usage_refresh(manual) {
        append_internal_app_log(
            "info",
            "accounts",
            "refresh-usage",
            "额度刷新请求返回",
            Some(format!(
                "method=GET, url={}, account={}, status={}",
                url,
                mask_secret(account_id),
                status
            )),
        );
    }
    if !(200..300).contains(&status) {
        append_account_usage_router_log(url, account_id, &status.to_string(), None, Some(&body));
        *collect_error = Some(format!(
            "method=GET, url={}, account={}, status={}",
            url,
            mask_secret(account_id),
            status
        ));
        if should_log_verbose_account_usage_refresh(manual) {
            append_internal_app_log(
                "warn",
                "accounts",
                "refresh-usage",
                "额度接口返回非成功状态",
                Some(format!(
                    "method=GET, url={}, account={}, status={}",
                    url,
                    mask_secret(account_id),
                    status
                )),
            );
        }
        return None;
    }

    if should_log_verbose_account_usage_refresh(manual) {
        append_internal_app_log(
            "info",
            "accounts",
            "refresh-usage",
            "额度刷新响应内容",
            Some(format!(
                "method=GET, url={}, account={}, body={}",
                url,
                mask_secret(account_id),
                truncate_text(&body, 1200)
            )),
        );
    }
    Some(body)
}
