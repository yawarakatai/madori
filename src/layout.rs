use crate::config::{Config, LayoutSpec, Rule};
use crate::detect::{ConnectorInfo, VideoMode};
use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedLayout {
    pub monitors: Vec<ResolvedMonitor>,
    pub virtual_output: Option<VirtualOutput>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedMonitor {
    pub monitor_name: String,
    pub connector_name: String,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: String,
    pub mirror: Option<String>,
    pub mode: Option<VideoMode>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VirtualOutput {
    pub width: u32,
    pub height: u32,
    pub refresh: f64,
}

pub fn resolve_layout(
    config: &Config,
    rule: &Rule,
    matched: &HashMap<String, String>,
    connectors: &[ConnectorInfo],
    wildcard_assignments: &HashMap<String, String>,
) -> ResolvedLayout {
    let mut resolved_monitors = Vec::new();
    let virtual_output = rule.virtual_output.as_ref().map(|v| VirtualOutput {
        width: v.width,
        height: v.height,
        refresh: v.refresh,
    });

    if let Some(ref layout) = rule.layout {
        // Phase 1: collect layout specs with resolved positions
        let mut entries: Vec<(String, &LayoutSpec, Option<(i32, i32)>)> = Vec::new();

        for (name, spec) in layout {
            let position = parse_position(spec.position.as_deref());
            entries.push((name.to_owned(), spec, position));
        }

        // Phase 2: compute auto positions
        // First, collect manual positions and compute their right edges
        let mut placed: HashMap<String, (i32, i32, i32)> = HashMap::new(); // name -> (x, y, right_x)

        for (name, spec, pos) in &entries {
            if let Some((x, y)) = pos {
                let width = get_monitor_width(name, spec, connectors, matched, wildcard_assignments);
                placed.insert(name.clone(), (*x, *y, *x + width));
            }
        }

        // Phase 3: auto-position remaining monitors
        for (name, spec, pos) in &entries {
            let (final_x, final_y) = if let Some((x, y)) = pos {
                (*x, *y)
            } else if let Some(ref position_str) = spec.position {
                if position_str.starts_with("auto") {
                    // "auto,y" format
                    let parts: Vec<&str> = position_str.split(',').collect();
                    let y: i32 = if parts.len() > 1 {
                        parts[1].parse().unwrap_or(0)
                    } else {
                        0
                    };
                    let max_right = placed.values().map(|(_, _, r)| *r).max().unwrap_or(0);
                    let x = max_right;
                    let right_x = x
                        + get_monitor_width(
                            name,
                            spec,
                            connectors,
                            matched,
                            wildcard_assignments,
                        );
                    placed.insert(name.clone(), (x, y, right_x));
                    (x, y)
                } else {
                    // No position specified
                    let y = 0;
                    let max_right = placed.values().map(|(_, _, r)| *r).max().unwrap_or(0);
                    let x = max_right;
                    let right_x = x
                        + get_monitor_width(
                            name,
                            spec,
                            connectors,
                            matched,
                            wildcard_assignments,
                        );
                    placed.insert(name.clone(), (x, y, right_x));
                    (x, y)
                }
            } else {
                (0, 0)
            };

            let connector = resolve_connector(name, matched, wildcard_assignments);
            let scale = spec
                .scale
                .or_else(|| {
                    config
                        .monitors
                        .get(name)
                        .and_then(|m| m.scale)
                })
                .unwrap_or(1.0);
            let transform = spec
                .transform
                .clone()
                .unwrap_or_else(|| "normal".to_string());
            let mirror = spec.mirror.clone();
            let mode = spec
                .mode
                .as_deref()
                .and_then(parse_mode_string)
                .or_else(|| get_mode_for_monitor(name, connectors, matched, wildcard_assignments));
            let enabled = spec.enabled.unwrap_or(true);

            resolved_monitors.push(ResolvedMonitor {
                monitor_name: name.clone(),
                connector_name: connector,
                x: final_x,
                y: final_y,
                scale,
                transform,
                mirror,
                mode,
                enabled,
            });
        }
    }

    ResolvedLayout {
        monitors: resolved_monitors,
        virtual_output,
    }
}

fn resolve_connector(
    name: &str,
    matched: &HashMap<String, String>,
    wildcards: &HashMap<String, String>,
) -> String {
    if let Some(conn) = matched.iter().find(|(_, v)| *v == name) {
        conn.0.clone()
    } else if let Some(conn) = wildcards.get(name) {
        conn.clone()
    } else {
        name.to_string()
    }
}

fn get_monitor_width(
    name: &str,
    spec: &LayoutSpec,
    connectors: &[ConnectorInfo],
    matched: &HashMap<String, String>,
    wildcards: &HashMap<String, String>,
) -> i32 {
    let connector = resolve_connector(name, matched, wildcards);
    let scale = spec.scale.unwrap_or(1.0);

    if let Some(ci) = connectors.iter().find(|c| c.name == connector) {
        if let Some(ref edid) = ci.edid {
            if let Some((w_mm, _)) = edid.display_size_mm {
                // Use physical size as a rough estimate, scaled
                return (w_mm as f64 / scale) as i32;
            }
        }
        // Fallback: use first mode's width
        if let Some(mode) = ci.modes.first() {
            return (mode.width as f64 / scale) as i32;
        }
    }
    1920 // Fallback width
}

fn parse_position(pos: Option<&str>) -> Option<(i32, i32)> {
    let pos = pos?;
    if pos.starts_with("auto") {
        return None; // Handle auto positioning later
    }
    let parts: Vec<&str> = pos.split(',').collect();
    if parts.len() == 2 {
        let x = parts[0].parse::<i32>().ok()?;
        let y = parts[1].parse::<i32>().ok()?;
        Some((x, y))
    } else {
        None
    }
}

fn get_mode_for_monitor(
    name: &str,
    connectors: &[ConnectorInfo],
    matched: &HashMap<String, String>,
    wildcards: &HashMap<String, String>,
) -> Option<VideoMode> {
    let connector_name = resolve_connector(name, matched, wildcards);
    connectors
        .iter()
        .find(|c| c.name == connector_name)
        .and_then(|c| c.modes.first().cloned())
}

fn parse_mode_string(s: &str) -> Option<VideoMode> {
    let (wh, refresh) = if let Some((wh, r)) = s.split_once('@') {
        (wh, r.parse::<f64>().ok()?)
    } else {
        (s, 60.0)
    };
    let (w, h) = wh.split_once('x')?;
    Some(VideoMode {
        width: w.parse().ok()?,
        height: h.parse().ok()?,
        refresh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{MatchBy, MonitorDefinition, LayoutSpec as CfgLayoutSpec, Rule as CfgRule};

    impl CfgRule {
        fn new(
            match_patterns: Vec<&str>,
            layout: Option<HashMap<String, CfgLayoutSpec>>,
            virtual_output: Option<crate::config::VirtualSpec>,
        ) -> Self {
            crate::config::Rule {
                match_patterns: match_patterns.iter().map(|s| s.to_string()).collect(),
                layout,
                virtual_output,
                pre_hook: None,
                post_hook: None,
            }
        }
    }

    fn conn(name: &str, modes: Vec<(u32, u32)>) -> ConnectorInfo {
        ConnectorInfo {
            name: name.to_string(),
            connected: true,
            edid: None,
            modes: modes
                .into_iter()
                .map(|(w, h)| VideoMode { width: w, height: h, refresh: 60.0 })
                .collect(),
        }
    }

    fn ls(pos: &str) -> CfgLayoutSpec {
        CfgLayoutSpec { position: Some(pos.to_string()), scale: None, transform: None, mirror: None, enabled: None, mode: None }
    }

    #[test]
    fn manual_position_single_monitor() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), ls("0,0"))])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        let m = &resolved.monitors[0];
        assert_eq!(m.monitor_name, "ally");
        assert_eq!(m.x, 0);
        assert_eq!(m.y, 0);
    }

    #[test]
    fn manual_position_custom_coords() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["innocn"],
            Some(HashMap::from([("innocn".into(), ls("1920,50"))])),
            None,
        );
        let matched = HashMap::from([("HDMI-A-1".into(), "innocn".to_string())]);
        let connectors = vec![conn("HDMI-A-1", vec![(3840, 2160)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert_eq!(resolved.monitors[0].x, 1920);
        assert_eq!(resolved.monitors[0].y, 50);
    }

    #[test]
    fn auto_position_packs_left_to_right() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally", "innocn"],
            Some(HashMap::from([
                ("ally".into(), ls("0,0")),
                ("innocn".into(), ls("auto,0")),
            ])),
            None,
        );
        let matched = HashMap::from([
            ("eDP-1".into(), "ally".to_string()),
            ("HDMI-A-1".into(), "innocn".to_string()),
        ]);
        // eDP-1 has width 1920 / scale 1.0 = 1920
        let connectors = vec![
            conn("eDP-1", vec![(1920, 1080)]),
            conn("HDMI-A-1", vec![(3840, 2160)]),
        ];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        let ally = resolved.monitors.iter().find(|m| m.monitor_name == "ally").unwrap();
        let innocn = resolved.monitors.iter().find(|m| m.monitor_name == "innocn").unwrap();
        assert_eq!((ally.x, ally.y), (0, 0));
        assert_eq!(innocn.x, 1920); // right edge of ally
        assert_eq!(innocn.y, 0);
    }

    #[test]
    fn auto_position_respects_custom_y() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally", "innocn"],
            Some(HashMap::from([
                ("ally".into(), ls("0,0")),
                ("innocn".into(), ls("auto,100")),
            ])),
            None,
        );
        let matched = HashMap::from([
            ("eDP-1".into(), "ally".to_string()),
            ("HDMI-A-1".into(), "innocn".to_string()),
        ]);
        let connectors = vec![
            conn("eDP-1", vec![(1920, 1080)]),
            conn("HDMI-A-1", vec![(3840, 2160)]),
        ];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        let innocn = resolved.monitors.iter().find(|m| m.monitor_name == "innocn").unwrap();
        assert_eq!(innocn.y, 100);
    }

    #[test]
    fn scale_inherits_from_monitor_definition() {
        let config = Config {
            monitors: HashMap::from([(
                "ally".into(),
                MonitorDefinition {
                    match_by: MatchBy { connector: None, model: None, vendor: None, serial: None },
                    scale: Some(1.5),
                },
            )]),
            rules: vec![],
        };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), ls("0,0"))])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert_eq!(resolved.monitors[0].scale, 1.5);
    }

    #[test]
    fn layout_scale_overrides_monitor_definition() {
        let config = Config {
            monitors: HashMap::from([(
                "ally".into(),
                MonitorDefinition {
                    match_by: MatchBy { connector: None, model: None, vendor: None, serial: None },
                    scale: Some(1.5),
                },
            )]),
            rules: vec![],
        };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), {
                let mut l = ls("0,0");
                l.scale = Some(2.0);
                l
            })])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert_eq!(resolved.monitors[0].scale, 2.0);
    }

    #[test]
    fn default_scale_is_one() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), ls("0,0"))])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert_eq!(resolved.monitors[0].scale, 1.0);
    }

    #[test]
    fn default_transform_is_normal() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), ls("0,0"))])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert_eq!(resolved.monitors[0].transform, "normal");
    }

    #[test]
    fn transform_passed_through() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), {
                let mut l = ls("0,0");
                l.transform = Some("right".into());
                l
            })])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert_eq!(resolved.monitors[0].transform, "right");
    }

    #[test]
    fn mirror_assignment() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally", "_"],
            Some(HashMap::from([
                ("ally".into(), ls("0,0")),
                ("$1".into(), {
                    let mut l = ls("0,0");
                    l.mirror = Some("ally".into());
                    l
                }),
            ])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![
            conn("eDP-1", vec![(1920, 1080)]),
            conn("HDMI-A-1", vec![(1920, 1080)]),
        ];
        let wildcards = HashMap::from([("$1".into(), "HDMI-A-1".to_string())]);
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &wildcards);
        let mirror_monitor = resolved.monitors.iter().find(|m| m.monitor_name == "$1").unwrap();
        assert_eq!(mirror_monitor.mirror.as_deref(), Some("ally"));
        assert_eq!(mirror_monitor.connector_name, "HDMI-A-1");
    }

    #[test]
    fn default_enabled_is_true() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), ls("0,0"))])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert!(resolved.monitors[0].enabled);
    }

    #[test]
    fn explicit_disabled() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["innocn"],
            Some(HashMap::from([("innocn".into(), {
                let mut l = ls("0,0");
                l.enabled = Some(false);
                l
            })])),
            None,
        );
        let matched = HashMap::from([("HDMI-A-1".into(), "innocn".to_string())]);
        let connectors = vec![conn("HDMI-A-1", vec![(3840, 2160)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert!(!resolved.monitors[0].enabled);
    }

    #[test]
    fn explicit_enabled_true() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), {
                let mut l = ls("0,0");
                l.enabled = Some(true);
                l
            })])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        assert!(resolved.monitors[0].enabled);
    }

    #[test]
    fn explicit_mode_overrides_detected() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["innocn"],
            Some(HashMap::from([("innocn".into(), {
                let mut l = ls("0,0");
                l.mode = Some("2560x1440@144".into());
                l
            })])),
            None,
        );
        let matched = HashMap::from([("HDMI-A-1".into(), "innocn".to_string())]);
        let connectors = vec![conn("HDMI-A-1", vec![(3840, 2160), (2560, 1440)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        let mode = resolved.monitors[0].mode.as_ref().unwrap();
        assert_eq!(mode.width, 2560);
        assert_eq!(mode.height, 1440);
        assert!((mode.refresh - 144.0).abs() < 1.0);
    }

    #[test]
    fn mode_defaults_to_first_detected() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), ls("0,0"))])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(1920, 1080), (1280, 720)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        let mode = resolved.monitors[0].mode.as_ref().unwrap();
        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);
    }

    #[test]
    fn mode_without_refresh_defaults_to_60() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), {
                let mut l = ls("0,0");
                l.mode = Some("1920x1080".into());
                l
            })])),
            None,
        );
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        let connectors = vec![conn("eDP-1", vec![(3840, 2160)])];
        let resolved = resolve_layout(&config, &rule, &matched, &connectors, &HashMap::new());
        let mode = resolved.monitors[0].mode.as_ref().unwrap();
        assert_eq!(mode.width, 1920);
        assert_eq!(mode.height, 1080);
        assert!((mode.refresh - 60.0).abs() < 1.0);
    }

    #[test]
    fn virtual_output_passed_through() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["*"],
            None,
            Some(crate::config::VirtualSpec { width: 1920, height: 1080, refresh: 60.0 }),
        );
        let resolved = resolve_layout(&config, &rule, &HashMap::new(), &[], &HashMap::new());
        let v = resolved.virtual_output.unwrap();
        assert_eq!(v.width, 1920);
        assert_eq!(v.height, 1080);
        assert_eq!(v.refresh, 60.0);
    }

    #[test]
    fn no_virtual_when_rule_has_none() {
        let config = Config { monitors: HashMap::new(), rules: vec![] };
        let rule = CfgRule::new(
            vec!["ally"],
            Some(HashMap::from([("ally".into(), ls("0,0"))])),
            None,
        );
        let resolved = resolve_layout(&config, &rule, &HashMap::new(), &[], &HashMap::new());
        assert!(resolved.virtual_output.is_none());
    }

    #[test]
    fn resolve_connector_from_matched() {
        let matched = HashMap::from([("eDP-1".into(), "ally".to_string())]);
        assert_eq!(resolve_connector("ally", &matched, &HashMap::new()), "eDP-1");
    }

    #[test]
    fn resolve_connector_from_wildcard() {
        let wildcards = HashMap::from([("$1".into(), "HDMI-A-1".to_string())]);
        assert_eq!(resolve_connector("$1", &HashMap::new(), &wildcards), "HDMI-A-1");
    }

    #[test]
    fn resolve_connector_fallback_to_name() {
        assert_eq!(resolve_connector("unknown", &HashMap::new(), &HashMap::new()), "unknown");
    }
}
