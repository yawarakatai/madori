use crate::adapter::Adapter;
use crate::layout::ResolvedLayout;
use log::{info, warn};
use std::process::Command;

pub struct NiriAdapter;

impl Adapter for NiriAdapter {
    fn detect() -> Option<Box<dyn Adapter>> {
        Some(Box::new(NiriAdapter))
    }

    fn apply(&self, layout: &ResolvedLayout) -> Result<(), Box<dyn std::error::Error>> {
        // Apply virtual output first if present
        if let Some(ref v) = layout.virtual_output {
            self.apply_virtual_output(v)?;
        }

        for monitor in &layout.monitors {
            let conn = &monitor.connector_name;

            if !monitor.enabled {
                info!("Disabling output {}", conn);
                let _ = niri_msg(&["output", conn, "off"]);
                continue;
            }

            if monitor.mirror.is_some() {
                warn!(
                    "niri does not support monitor mirroring; skipping mirror for {}",
                    monitor.monitor_name
                );
                continue;
            }

            // Set mode (preferred mode closest to desired refresh)
            if let Some(ref mode) = monitor.mode {
                let _ = niri_msg(&["output", conn, "mode",
                    &mode.width.to_string(),
                    &mode.height.to_string(),
                    &format!("{:.3}", mode.refresh),
                ]);
            }

            // Set position
            let _ = niri_msg(&[
                "output",
                conn,
                "position",
                &monitor.x.to_string(),
                &monitor.y.to_string(),
            ]);

            // Set scale
            let _ = niri_msg(&["output", conn, "scale", &format!("{:.3}", monitor.scale)]);

            // Set transform
            if monitor.transform != "normal" {
                let niri_transform = match monitor.transform.as_str() {
                    "left" => "90",
                    "right" => "270",
                    "inverted" => "180",
                    _ => "normal",
                };
                let _ = niri_msg(&["output", conn, "transform", niri_transform]);
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "niri"
    }
}

impl NiriAdapter {
    fn apply_virtual_output(
        &self,
        v: &crate::layout::VirtualOutput,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = niri_msg(&["output", "Virtual-1", "on"]);
        let _ = niri_msg(&[
            "output",
            "Virtual-1",
            "mode",
            &v.width.to_string(),
            &v.height.to_string(),
            &format!("{:.3}", v.refresh),
        ]);
        Ok(())
    }
}

fn niri_msg(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("niri")
        .arg("msg")
        .args(args)
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            info!("niri msg {}: {}", args.join(" "), stdout.trim());
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!("niri msg {} failed: {}", args.join(" "), stderr.trim());
    }

    Ok(())
}
