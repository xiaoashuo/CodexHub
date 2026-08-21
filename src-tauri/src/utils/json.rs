use crate::*;
use crate::constants::*;
use std::collections::HashSet;

pub(crate) fn first_non_empty_value(values: &[String]) -> Option<String> {
    values
        .iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn find_string_by_keys(value: &serde_json::Value, keys: &[&str]) -> Option<String> {
    match value {
        serde_json::Value::Object(map) => {
            for key in keys {
                if let Some(found) = map
                    .get(*key)
                    .and_then(|value| value.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    return Some(found.to_string());
                }
            }

            for child in map.values() {
                if let Some(found) = find_string_by_keys(child, keys) {
                    return Some(found);
                }
            }

            None
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if let Some(found) = find_string_by_keys(item, keys) {
                    return Some(found);
                }
            }

            None
        }
        _ => None,
    }
}

pub(crate) fn ensure_json_object(value: &mut serde_json::Value) {
    if !value.is_object() {
        *value = serde_json::json!({});
    }
}

pub(crate) fn json_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(ToString::to_string))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn unique_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for value in values {
        if seen.insert(value.clone()) {
            unique.push(value);
        }
    }
    unique
}

pub(crate) fn front_unique(existing: Vec<String>, front: Vec<String>) -> Vec<String> {
    let mut combined = Vec::with_capacity(existing.len() + front.len());
    combined.extend(front);
    combined.extend(existing);
    unique_strings(combined)
}

pub(crate) fn append_unique(existing: Vec<String>, additions: Vec<String>) -> Vec<String> {
    let mut combined = Vec::with_capacity(existing.len() + additions.len());
    combined.extend(existing);
    combined.extend(additions);
    unique_strings(combined)
}

pub(crate) fn merge_ordered_unique(
    existing: Vec<String>,
    additions: Vec<String>,
    move_to_recent: bool,
) -> Vec<String> {
    if move_to_recent {
        front_unique(existing, additions)
    } else {
        append_unique(existing, additions)
    }
}

pub(crate) fn json_string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn json_number_or_string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value.get(key).and_then(|item| {
        if let Some(text) = item.as_str() {
            return Some(text.to_string());
        }
        if let Some(number) = item.as_i64() {
            return Some(number.to_string());
        }
        item.as_u64().map(|number| number.to_string())
    })
}

pub(crate) fn json_u64_number_field(value: &serde_json::Value, key: &str) -> Option<u64> {
    value.get(key).and_then(|item| {
        if let Some(number) = item.as_u64() {
            return Some(number);
        }
        item.as_str()?.trim().parse::<u64>().ok()
    })
}

pub(crate) fn find_u8_by_keys(value: &serde_json::Value, keys: &[&str]) -> Option<u8> {
    for key in keys {
        if let Some(number) = value.get(*key).and_then(|item| item.as_u64()) {
            return Some(number.min(100) as u8);
        }
    }

    None
}

pub(crate) fn json_string(value: &str) -> String {
    serde_json::Value::String(value.to_string()).to_string()
}
