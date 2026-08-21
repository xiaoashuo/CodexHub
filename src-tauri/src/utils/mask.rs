use crate::*;
use crate::constants::*;

pub(crate) fn mask_secret(secret: &str) -> String {
    let trimmed = secret.trim();
    if trimmed.len() <= 12 {
        return "****".to_string();
    }

    format!("{}...{}", &trimmed[..8], &trimmed[trimmed.len() - 4..])
}

pub(crate) fn is_real_user_text(text: &str) -> bool {
    !text.trim_start().starts_with("<environment_context>")
}

pub(crate) fn truncate_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized.chars().count() <= max_chars {
        return normalized;
    }

    format!(
        "{}...",
        normalized.chars().take(max_chars).collect::<String>()
    )
}

pub(crate) fn redact_sensitive_text(text: &str) -> String {
    let mut result = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("authorization:")
            || lower.contains("cookie:")
            || lower.contains("api-key:")
            || lower.contains("apikey:")
            || lower.contains("x-api-key:")
        {
            let prefix = line
                .split_once(':')
                .map(|(prefix, _)| prefix)
                .unwrap_or(line);
            result.push(format!("{}: <redacted>", prefix));
        } else {
            result.push(line.to_string());
        }
    }
    result.join("\n")
}

pub(crate) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
