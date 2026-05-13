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

        // Count wildcards in patterns (exclude $N references)
        let wildcard_count = patterns.iter().filter(|p| p.as_str() == "_").count();
        let known_spec_count = patterns
            .iter()
            .filter(|p| p.as_str() != "_" && p.as_str() != "*" && !p.starts_with('$'))
            .count();

        // Must have enough connected monitors to satisfy known + wildcards
        let required_count = known_spec_count + wildcard_count;
        if required_count > connected_count {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LayoutSpec, MatchBy, MonitorDefinition, Rule, VirtualSpec};
    use crate::detect::{ConnectorInfo, EdidInfo};

    fn conn(name: &str, connected: bool, model: Option<&str>, vendor: Option<&str>, serial: Option<&str>) -> ConnectorInfo {
        let edid = if model.is_some() || vendor.is_some() || serial.is_some() {
            Some(EdidInfo {
                model: model.map(|s| s.to_string()),
                vendor: vendor.map(|s| s.to_string()),
                serial: serial.map(|s| s.to_string()),
                display_size_mm: None,
                preferred_mode: None,
            })
        } else {
            None
        };
        ConnectorInfo {
            name: name.to_string(),
            connected,
            edid,
            modes: vec![],
        }
    }

    // -- monitor matching tests --

    #[test]
    fn match_by_connector_exact() {
        let monitors = HashMap::from([(
            "ally".into(),
            MonitorDefinition {
                match_by: MatchBy {
                    connector: Some("eDP-1".into()),
                    model: None,
                    vendor: None,
                    serial: None,
                },
                scale: None,
            },
        )]);
        let connectors = vec![conn("eDP-1", true, None, None, None)];
        let matched = match_monitors(&monitors, &connectors);
        assert_eq!(matched.get("eDP-1"), Some(&"ally".to_string()));
    }

    #[test]
    fn match_by_connector_not_matched() {
        let monitors = HashMap::from([(
            "ally".into(),
            MonitorDefinition {
                match_by: MatchBy { connector: Some("HDMI-A-1".into()), model: None, vendor: None, serial: None },
                scale: None,
            },
        )]);
        let connectors = vec![conn("eDP-1", true, None, None, None)];
        let matched = match_monitors(&monitors, &connectors);
        assert!(matched.is_empty());
    }

    #[test]
    fn match_by_model_contains() {
        let monitors = HashMap::from([(
            "innocn".into(),
            MonitorDefinition {
                match_by: MatchBy {
                    connector: None,
                    model: Some("32M2V".into()),
                    vendor: None,
                    serial: None,
                },
                scale: None,
            },
        )]);
        let connectors = vec![conn("HDMI-A-1", true, Some("32M2V"), None, None)];
        let matched = match_monitors(&monitors, &connectors);
        assert_eq!(matched.get("HDMI-A-1"), Some(&"innocn".to_string()));
    }

    #[test]
    fn match_by_vendor_exact() {
        let monitors = HashMap::from([(
            "projector".into(),
            MonitorDefinition {
                match_by: MatchBy { connector: None, model: None, vendor: Some("EPS".into()), serial: None },
                scale: None,
            },
        )]);
        let connectors = vec![conn("HDMI-A-1", true, Some("EH-TW7100"), Some("EPS"), None)];
        let matched = match_monitors(&monitors, &connectors);
        assert_eq!(matched.get("HDMI-A-1"), Some(&"projector".to_string()));
    }

    #[test]
    fn match_by_vendor_wrong() {
        let monitors = HashMap::from([(
            "projector".into(),
            MonitorDefinition {
                match_by: MatchBy { connector: None, model: None, vendor: Some("EPS".into()), serial: None },
                scale: None,
            },
        )]);
        let connectors = vec![conn("HDMI-A-1", true, Some("32M2V"), Some("IOC"), None)];
        let matched = match_monitors(&monitors, &connectors);
        assert!(matched.is_empty());
    }

    #[test]
    fn match_by_connector_and_model() {
        let monitors = HashMap::from([(
            "innocn".into(),
            MonitorDefinition {
                match_by: MatchBy {
                    connector: Some("HDMI-A-1".into()),
                    model: Some("32M2V".into()),
                    vendor: None,
                    serial: None,
                },
                scale: None,
            },
        )]);
        let wrong_conn = vec![conn("DP-1", true, Some("32M2V"), None, None)];
        assert!(match_monitors(&monitors, &wrong_conn).is_empty());

        let wrong_model = vec![conn("HDMI-A-1", true, Some("Other"), None, None)];
        assert!(match_monitors(&monitors, &wrong_model).is_empty());

        let correct = vec![conn("HDMI-A-1", true, Some("32M2V"), None, None)];
        assert_eq!(match_monitors(&monitors, &correct).get("HDMI-A-1"), Some(&"innocn".to_string()));
    }

    #[test]
    fn match_disconnected_monitor_ignored() {
        let monitors = HashMap::from([(
            "ally".into(),
            MonitorDefinition {
                match_by: MatchBy { connector: Some("eDP-1".into()), model: None, vendor: None, serial: None },
                scale: None,
            },
        )]);
        let connectors = vec![conn("eDP-1", false, None, None, None)];
        assert!(match_monitors(&monitors, &connectors).is_empty());
    }

    #[test]
    fn match_multiple_monitors() {
        let monitors = HashMap::from([
            ("ally".into(), MonitorDefinition {
                match_by: MatchBy { connector: Some("eDP-1".into()), model: None, vendor: None, serial: None },
                scale: None,
            }),
            ("innocn".into(), MonitorDefinition {
                match_by: MatchBy { connector: None, model: Some("32M2V".into()), vendor: None, serial: None },
                scale: None,
            }),
        ]);
        let connectors = vec![
            conn("eDP-1", true, None, None, None),
            conn("HDMI-A-1", true, Some("32M2V"), None, None),
        ];
        let matched = match_monitors(&monitors, &connectors);
        assert_eq!(matched.len(), 2);
        assert_eq!(matched.get("eDP-1"), Some(&"ally".to_string()));
        assert_eq!(matched.get("HDMI-A-1"), Some(&"innocn".to_string()));
    }

    // -- rule matching tests --

    fn mk_rule(match_patterns: Vec<&str>, layout: Option<HashMap<String, LayoutSpec>>, virtual_output: Option<VirtualSpec>) -> Rule {
        Rule {
            match_patterns: match_patterns.iter().map(|s| s.to_string()).collect(),
            layout,
            virtual_output,
        }
    }

    fn layout_spec(position: &str) -> LayoutSpec {
        LayoutSpec { position: Some(position.to_string()), scale: None, transform: None, mirror: None, enabled: None }
    }

    #[test]
    fn rule_match_single_known() {
        let rules = vec![mk_rule(vec!["ally"], Some(HashMap::from([("ally".into(), layout_spec("0,0"))])), None)];
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", true, None, None, None)];
        let result = match_rules(&rules, &matched, &connectors);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, 0);
    }

    #[test]
    fn rule_no_match_when_monitor_missing() {
        let rules = vec![mk_rule(vec!["ally", "innocn"], None, None)];
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", true, None, None, None)];
        assert!(match_rules(&rules, &matched, &connectors).is_none());
    }

    #[test]
    fn rule_match_wildcard_any_unknown() {
        let rules = vec![mk_rule(vec!["ally", "_"], None, None)];
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![
            conn("eDP-1", true, None, None, None),
            conn("HDMI-A-1", true, Some("Unknown"), Some("XYZ"), None),
        ];
        let result = match_rules(&rules, &matched, &connectors);
        assert!(result.is_some());
    }

    #[test]
    fn rule_match_wildcard_no_unknown() {
        let rules = vec![mk_rule(vec!["ally", "_"], None, None)];
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", true, None, None, None)];
        // Need one unknown but none exists
        assert!(match_rules(&rules, &matched, &connectors).is_none());
    }

    #[test]
    fn rule_catch_all_always_matches() {
        let rules = vec![mk_rule(vec!["*"], None, Some(VirtualSpec { width: 1920, height: 1080, refresh: 60.0 }))];
        let matched = HashMap::new();
        let connectors: Vec<ConnectorInfo> = vec![];
        let result = match_rules(&rules, &matched, &connectors);
        assert!(result.is_some());
        assert!(result.unwrap().1.virtual_output.is_some());
    }

    #[test]
    fn rule_first_match_wins() {
        let rules = vec![
            mk_rule(vec!["ally"], Some(HashMap::from([("ally".into(), layout_spec("0,0"))])), None),
            mk_rule(vec!["ally"], Some(HashMap::from([("ally".into(), layout_spec("100,100"))])), None),
        ];
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", true, None, None, None)];
        let (i, r) = match_rules(&rules, &matched, &connectors).unwrap();
        assert_eq!(i, 0);
        assert_eq!(r.layout.unwrap().get("ally").unwrap().position.as_deref(), Some("0,0"));
    }

    #[test]
    fn rule_too_many_patterns_no_match() {
        let rules = vec![mk_rule(vec!["ally", "innocn", "projector"], None, None)];
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", true, None, None, None)];
        assert!(match_rules(&rules, &matched, &connectors).is_none());
    }

    #[test]
    fn rule_with_dollar_ref_matches() {
        let rules = vec![mk_rule(vec!["ally", "_", "$1"], None, None)];
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![
            conn("eDP-1", true, None, None, None),
            conn("HDMI-A-1", true, Some("X"), Some("Y"), None),
        ];
        let result = match_rules(&rules, &matched, &connectors);
        assert!(result.is_some());
    }

    // -- resolve_wildcards tests --

    #[test]
    fn resolve_wildcards_maps_unknown_to_dollar() {
        let rule = mk_rule(vec!["ally", "_"], None, None);
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![
            conn("eDP-1", true, None, None, None),
            conn("HDMI-A-1", true, Some("Foo"), Some("BAR"), None),
        ];
        let assignments = resolve_wildcards(&rule, &matched, &connectors);
        assert_eq!(assignments.get("$1"), Some(&"HDMI-A-1".to_string()));
    }

    #[test]
    fn resolve_wildcards_multiple() {
        let rule = mk_rule(vec!["ally", "_", "_"], None, None);
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![
            conn("eDP-1", true, None, None, None),
            conn("HDMI-A-1", true, Some("A"), Some("B"), None),
            conn("DP-1", true, Some("C"), Some("D"), None),
        ];
        let assignments = resolve_wildcards(&rule, &matched, &connectors);
        assert_eq!(assignments.get("$1"), Some(&"HDMI-A-1".to_string()));
        assert_eq!(assignments.get("$2"), Some(&"DP-1".to_string()));
    }

    #[test]
    fn resolve_wildcards_with_dollar_ref() {
        let rule = mk_rule(vec!["ally", "_", "$1"], None, None);
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![
            conn("eDP-1", true, None, None, None),
            conn("HDMI-A-1", true, Some("Ext"), Some("AAA"), None),
        ];
        let assignments = resolve_wildcards(&rule, &matched, &connectors);
        assert_eq!(assignments.get("$1"), Some(&"HDMI-A-1".to_string()));
    }
}
