//! Provider selection is deliberately independent of HTTP forwarding.  It mirrors
//! waliapi's channel dispatcher: highest priority wins and routes in the same
//! priority tier are distributed with smooth weighted round-robin.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub(crate) struct DispatchCandidate {
    pub key: String,
    pub priority: i32,
    pub weight: u32,
}

/// Returns candidates in the order they should be attempted.  The first entry
/// is selected by smooth weighted round-robin; remaining entries provide a
/// deterministic failover order for callers that can retry safely.
pub(crate) fn order_candidates(
    route_key: &str,
    mut candidates: Vec<DispatchCandidate>,
) -> Vec<DispatchCandidate> {
    if candidates.is_empty() {
        return candidates;
    }

    let highest_priority = candidates.iter().map(|item| item.priority).max().unwrap_or(0);
    let mut preferred = Vec::new();
    let mut fallbacks = Vec::new();
    for candidate in candidates.drain(..) {
        if candidate.priority == highest_priority {
            preferred.push(candidate);
        } else {
            fallbacks.push(candidate);
        }
    }
    preferred.sort_by(|left, right| left.key.cmp(&right.key));

    let selected_index = select_weighted_index(route_key, &preferred);
    let selected = preferred.remove(selected_index);
    let mut ordered = vec![selected];
    preferred.sort_by(|left, right| right.weight.cmp(&left.weight).then_with(|| left.key.cmp(&right.key)));
    ordered.extend(preferred);
    // Lower priority tiers are not used for normal balancing, but are retained
    // as a safe retry queue for forwarding paths that can fail over.
    fallbacks.sort_by(|left, right| right.priority.cmp(&left.priority).then_with(|| right.weight.cmp(&left.weight)).then_with(|| left.key.cmp(&right.key)));
    ordered.extend(fallbacks);
    ordered
}

fn select_weighted_index(route_key: &str, candidates: &[DispatchCandidate]) -> usize {
    if candidates.len() <= 1 {
        return 0;
    }

    let states = smooth_weights();
    let mut states = states.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let entry = states.entry(route_key.to_string()).or_default();
    entry.retain(|key, _| candidates.iter().any(|candidate| &candidate.key == key));

    let total_weight: i64 = candidates.iter().map(|candidate| candidate.weight.max(1) as i64).sum();
    let mut selected_index = 0;
    let mut selected_score = i64::MIN;
    for (index, candidate) in candidates.iter().enumerate() {
        let score = entry.entry(candidate.key.clone()).or_insert(0);
        *score += candidate.weight.max(1) as i64;
        if *score > selected_score {
            selected_score = *score;
            selected_index = index;
        }
    }
    if let Some(score) = entry.get_mut(&candidates[selected_index].key) {
        *score -= total_weight;
    }
    selected_index
}

fn smooth_weights() -> &'static Mutex<HashMap<String, HashMap<String, i64>>> {
    static WEIGHTS: OnceLock<Mutex<HashMap<String, HashMap<String, i64>>>> = OnceLock::new();
    WEIGHTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_tier_excludes_lower_priority_routes() {
        let candidates = vec![
            DispatchCandidate { key: "primary".into(), priority: 10, weight: 1 },
            DispatchCandidate { key: "fallback".into(), priority: 1, weight: 100 },
        ];
        let ordered = order_candidates("priority-test", candidates);
        assert_eq!(ordered[0].key, "primary");
        assert_eq!(ordered[1].key, "fallback");
    }

    #[test]
    fn weighted_round_robin_honors_weight() {
        let candidates = vec![
            DispatchCandidate { key: "a".into(), priority: 0, weight: 1 },
            DispatchCandidate { key: "b".into(), priority: 0, weight: 3 },
        ];
        let picks = (0..8)
            .map(|_| order_candidates("weight-test", candidates.clone())[0].key.clone())
            .collect::<Vec<_>>();
        assert_eq!(picks.iter().filter(|key| key.as_str() == "b").count(), 6);
    }
}
