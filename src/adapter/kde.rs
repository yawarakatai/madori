use crate::adapter::Adapter;
use crate::layout::ResolvedLayout;
use log::{info, warn};
use std::process::Command;

pub struct KdeAdapter;

impl Adapter for KdeAdapter {
    fn detect() -> Option<Box<dyn Adapter>> {
        Some(Box::new(KdeAdapter))
    }

    fn apply(&self, layout: &ResolvedLayout) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref v) = layout.virtual_output {
            warn!(
                "Virtual output on KDE: creating virtual monitor {}x{}@{}",
                v.width, v.height, v.refresh
            );
            let _ = kscreen_doctor(&[
                &format!("output.HEADLESS-1.mode.{}x{}@{}", v.width, v.height, v.refresh as u32),
            ]);
            let _ = kscreen_doctor(&["output.HEADLESS-1.enable"]);
        }

        for monitor in &layout.monitors {
            let conn = &monitor.connector_name;

            if !monitor.enabled {
                info!("Disabling output {}", conn);
                let _ = kscreen_doctor(&[&format!("output.{}.disable", conn)]);
                continue;
            }

            if let Some(ref mirror_target) = monitor.mirror {
                let target_conn = layout
                    .monitors
                    .iter()
                    .find(|m| m.monitor_name == *mirror_target)
                    .map(|m| m.connector_name.as_str())
                    .unwrap_or(mirror_target.as_str());
                let _ = kscreen_doctor(&[&format!(
                    "output.{}.replicate.{}",
                    conn, target_conn
                )]);
                continue;
            }

            // Position
            let _ = kscreen_doctor(&[&format!(
                "output.{}.position.{},{}",
                conn, monitor.x, monitor.y
            )]);

            // Scale
            let _ = kscreen_doctor(&[&format!(
                "output.{}.scale.{}",
                conn, monitor.scale
            )]);

            // Rotation
            if monitor.transform != "normal" {
                let rotation = match monitor.transform.as_str() {
                    "left" => "left",
                    "right" => "right",
                    "inverted" => "inverted",
                    _ => "normal",
                };
                let _ =
                    kscreen_doctor(&[&format!("output.{}.rotation.{}", conn, rotation)]);
            }

            // Mode
            if let Some(ref mode) = monitor.mode {
                let _ = kscreen_doctor(&[&format!(
                    "output.{}.mode.{}x{}@{}",
                    conn, mode.width, mode.height, mode.refresh as u32
                )]);
            }

            // Enable
            let _ = kscreen_doctor(&[&format!("output.{}.enable", conn)]);
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "KDE"
    }
}

fn kscreen_doctor(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("kscreen-doctor").args(args).output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            info!("kscreen-doctor {}: {}", args.join(" "), stdout.trim());
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "kscreen-doctor {} failed: {}",
            args.join(" "),
            stderr.trim()
        );
    }

    Ok(())
}
