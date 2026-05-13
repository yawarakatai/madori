use crate::config::{Config, LayoutSpec, Rule};
use crate::detect::{ConnectorInfo, VideoMode};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ResolvedLayout {
    pub monitors: Vec<ResolvedMonitor>,
    pub virtual_output: Option<VirtualOutput>,
}

#[derive(Debug, Clone)]
pub struct ResolvedMonitor {
    pub monitor_name: String,
    pub connector_name: String,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub transform: String,
    pub mirror: Option<String>,
    pub mode: Option<VideoMode>,
}

#[derive(Debug, Clone)]
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
            let mode = get_mode_for_monitor(name, connectors, matched, wildcard_assignments);

            resolved_monitors.push(ResolvedMonitor {
                monitor_name: name.clone(),
                connector_name: connector,
                x: final_x,
                y: final_y,
                scale,
                transform,
                mirror,
                mode,
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
