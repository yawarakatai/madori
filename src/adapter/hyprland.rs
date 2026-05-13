use crate::adapter::Adapter;
use crate::layout::ResolvedLayout;
use log::{info, warn};
use std::process::Command;

pub struct HyprlandAdapter;

impl Adapter for HyprlandAdapter {
    fn detect() -> Option<Box<dyn Adapter>> {
        Some(Box::new(HyprlandAdapter))
    }

    fn apply(&self, layout: &ResolvedLayout) -> Result<(), Box<dyn std::error::Error>> {
        if !check_binary("hyprctl") {
            warn!("hyprctl not found in PATH; skipping application");
            return Ok(());
        }

        if let Some(ref v) = layout.virtual_output {
            let _ = hyprctl(&[
                "keyword", "monitor",
                &format!(
                    "HEADLESS-1,{}x{}@{},{},1,transform,0",
                    v.width, v.height, v.refresh as u32, "auto"
                ),
            ]);
        }

        for monitor in &layout.monitors {
            let conn = &monitor.connector_name;

            if !monitor.enabled {
                info!("Disabling output {}", conn);
                let _ = hyprctl(&["keyword", "monitor", &format!("{},disable", conn)]);
                continue;
            }

            let mode_str = if let Some(ref mode) = monitor.mode {
                format!("{}x{}@{:.0}", mode.width, mode.height, mode.refresh)
            } else {
                "preferred".to_string()
            };

            let pos_str = format!("{}x{}", monitor.x, monitor.y);
            let scale_str = format!("{:.2}", monitor.scale);
            let transform_num = match monitor.transform.as_str() {
                "normal" => "0",
                "left" => "1",
                "right" => "3",
                "inverted" => "2",
                _ => "0",
            };

            let mirror_str = if let Some(ref mirror_target) = monitor.mirror {
                let target_conn = layout
                    .monitors
                    .iter()
                    .find(|m| m.monitor_name == *mirror_target)
                    .map(|m| m.connector_name.as_str())
                    .unwrap_or(mirror_target.as_str());
                format!(",mirror,{}", target_conn)
            } else {
                String::new()
            };

            let monitor_config = format!(
                "{},{},{},{},transform,{}{}",
                conn, mode_str, pos_str, scale_str, transform_num, mirror_str
            );

            let _ = hyprctl(&["keyword", "monitor", &monitor_config]);
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "Hyprland"
    }
}

fn hyprctl(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("hyprctl").args(args).output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            info!("hyprctl {}: {}", args.join(" "), stdout.trim());
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("hyprctl {} failed: {}", args.join(" "), stderr.trim());
    }

    Ok(())
}

fn check_binary(name: &str) -> bool {
    match std::process::Command::new("which").arg(name).output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}
