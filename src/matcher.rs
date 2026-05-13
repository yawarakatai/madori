use crate::config::{MonitorDefinition, Rule};
use crate::detect::ConnectorInfo;
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MatchResult {
    pub matched_monitors: HashMap<String, String>,
    pub rule_index: Option<usize>,
    pub rule: Option<Rule>,
}

pub fn match_monitors(
    monitors: &HashMap<String, MonitorDefinition>,
    connectors: &[ConnectorInfo],
) -> HashMap<String, String> {
    let mut matched = HashMap::new();

    for connector in connectors.iter().filter(|c| c.connected) {
        let name = match_connector(monitors, connector);
        if let Some(monitor_name) = name {
            matched.insert(connector.name.clone(), monitor_name);
        }
    }

    matched
}

fn match_connector(
    monitors: &HashMap<String, MonitorDefinition>,
    connector: &ConnectorInfo,
) -> Option<String> {
    let edid = connector.edid.as_ref();

    for (name, def) in monitors {
        let mb = &def.match_by;

        // Check connector match
        if let Some(ref conn) = mb.connector {
            if conn != &connector.name {
                continue;
            }
        }

        // Check model match
        if let Some(ref model) = mb.model {
            match edid.and_then(|e| e.model.as_deref()) {
                Some(edid_model) if edid_model.contains(model.as_str()) => {}
                _ => continue,
            }
        }

        // Check vendor match
        if let Some(ref vendor) = mb.vendor {
            match edid.and_then(|e| e.vendor.as_deref()) {
                Some(edid_vendor) if edid_vendor == vendor.as_str() => {}
                _ => continue,
            }
        }

        // Check serial match
        if let Some(ref serial) = mb.serial {
            match edid.and_then(|e| e.serial.as_deref()) {
                Some(edid_serial) if edid_serial == serial.as_str() => {}
                _ => continue,
            }
        }

        return Some(name.clone());
    }

    None
}

pub fn match_rules(
    rules: &[Rule],
    matched: &HashMap<String, String>,
    connectors: &[ConnectorInfo],
) -> Option<(usize, Rule)> {
    let known_names: Vec<&String> = matched.values().collect();
    let connected_count = connectors.iter().filter(|c| c.connected).count();

    for (i, rule) in rules.iter().enumerate() {
        let patterns = &rule.match_patterns;

        // Handle catch-all "*"
        if patterns.len() == 1 && patterns[0] == "*" {
            // Only match catch-all if no other rule could match.
            // Actually, spec says first match wins. If we're the first rule and
            // no others match, we match. But since we evaluate top-to-bottom,
            // we'll check this only at the correct position.
            return Some((i, rule.clone()));
        }

        // Count wildcards in patterns
        let wildcard_count = patterns.iter().filter(|p| p.as_str() == "_").count();
        let known_spec_count = patterns
            .iter()
            .filter(|p| p.as_str() != "_" && p.as_str() != "*")
            .count();

        // Must have enough connected monitors to match
        if patterns.len() > connected_count {
            continue;
        }

        // Must match known monitor names
        let mut known_ok = true;
        for pat in patterns {
            if pat == "_" || pat == "*" {
                continue;
            }
            if pat.starts_with('$') {
                // Reference to a wildcard, resolve later
                continue;
            }
            // Literal monitor name - must be present
            if !known_names.contains(&pat) {
                known_ok = false;
                break;
            }
        }
        if !known_ok {
            continue;
        }

        // Check that every known spec monitor corresponds to a connected monitor
        // and that we have enough connectors to cover the total count
        if known_spec_count > known_names.len() {
            continue;
        }

        // Check wildcard matching: need enough unknown connectors to satisfy _
        let unknown_count = connectors
            .iter()
            .filter(|c| c.connected && !matched.contains_key(&c.name))
            .count();
        if wildcard_count > unknown_count + known_spec_count {
            // Not enough monitors total
            continue;
        }

        // More precise check: count how many matched monitors aren't referenced in patterns
        let referenced_known: Vec<&String> = patterns
            .iter()
            .filter(|p| p.as_str() != "_" && p.as_str() != "*" && !p.starts_with('$'))
            .collect();
        let extra_known = known_names.len() - referenced_known.len();
        if wildcard_count < extra_known {
            continue;
        }

        return Some((i, rule.clone()));
    }

    None
}

pub fn resolve_wildcards(
    rule: &Rule,
    matched: &HashMap<String, String>,
    connectors: &[ConnectorInfo],
) -> HashMap<String, String> {
    let mut assignments = HashMap::new();
    let wildcard_connectors: Vec<&String> = connectors
        .iter()
        .filter(|c| c.connected && !matched.contains_key(&c.name))
        .map(|c| &c.name)
        .collect();

    // Also add matched monitors not explicitly named in patterns
    let known_names: Vec<&String> = matched.values().collect();
    let named_in_pattern: Vec<&String> = rule
        .match_patterns
        .iter()
        .filter(|p| p.as_str() != "_" && p.as_str() != "*" && !p.starts_with('$'))
        .collect();

    let extra_known: Vec<&String> = known_names
        .iter()
        .filter(|n| !named_in_pattern.contains(n))
        .copied()
        .collect();

    // For extra known monitors, we need their connector names
    let mut extra_connectors: Vec<&String> = Vec::new();
    for ek in &extra_known {
        if let Some((conn, _)) = matched.iter().find(|(_, v)| v == ek) {
            extra_connectors.push(conn);
        }
    }

    let mut wi = 0;
    for pat in &rule.match_patterns {
        if pat == "_" {
            // Try extra_connectors first, then wildcard_connectors
            let conn = if wi < extra_connectors.len() {
                extra_connectors[wi].clone()
            } else if (wi - extra_connectors.len()) < wildcard_connectors.len() {
                wildcard_connectors[wi - extra_connectors.len()].clone()
            } else {
                continue;
            };
            assignments.insert(format!("${}", wi + 1), conn);
            wi += 1;
        } else if pat.starts_with('$') {
            // Reference is resolved later from layout
            // Actually, $N refers to the Nth wildcard's assigned connector
            // We need to find what connector the Nth wildcard got assigned to
            if let Some(idx_str) = pat.strip_prefix('$') {
                if let Ok(idx) = idx_str.parse::<usize>() {
                    // Find the connector for wildcard at index idx
                    let wc_idx = idx - 1;
                    let connector = if wc_idx < extra_connectors.len() {
                        extra_connectors[wc_idx].clone()
                    } else if (wc_idx - extra_connectors.len()) < wildcard_connectors.len() {
                        wildcard_connectors[wc_idx - extra_connectors.len()].clone()
                    } else {
                        continue;
                    };
                    assignments.insert(pat.clone(), connector);
                }
            }
        }
    }

    assignments
}
