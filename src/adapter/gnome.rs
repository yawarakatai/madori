use crate::adapter::Adapter;
use crate::layout::ResolvedLayout;
use log::{info, warn};
use std::process::Command;

pub struct GnomeAdapter;

impl Adapter for GnomeAdapter {
    fn detect() -> Option<Box<dyn Adapter>> {
        Some(Box::new(GnomeAdapter))
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
                let _ = gnome_randr(&["--output", conn, "--off"]);
                continue;
            }

            if let Some(ref mirror_target) = monitor.mirror {
                let target_conn = layout
                    .monitors
                    .iter()
                    .find(|m| m.monitor_name == *mirror_target)
                    .map(|m| m.connector_name.as_str())
                    .unwrap_or(mirror_target.as_str());
                let _ = gnome_randr(&[
                    "--output",
                    conn,
                    "--same-as",
                    target_conn,
                ]);
                continue;
            }

            // Set position
            let _ = gnome_randr(&[
                "--output",
                conn,
                "--pos",
                &format!("{}x{}", monitor.x, monitor.y),
            ]);

            // Set scale
            let _ = gnome_randr(&[
                "--output",
                conn,
                "--scale",
                &format!("{:.3}", monitor.scale),
            ]);

            // Set transform
            if monitor.transform != "normal" {
                let gnome_transform = match monitor.transform.as_str() {
                    "left" => "left",
                    "right" => "right",
                    "inverted" => "inverted",
                    _ => "normal",
                };
                let _ = gnome_randr(&["--output", conn, "--rotate", gnome_transform]);
            }

            // Set mode
            if let Some(ref mode) = monitor.mode {
                let mode_str = format!("{}x{}", mode.width, mode.height);
                let _ = gnome_randr(&["--output", conn, "--mode", &mode_str]);
                if mode.refresh != 60.0 {
                    let _ = gnome_randr(&[
                        "--output",
                        conn,
                        "--rate",
                        &format!("{:.3}", mode.refresh),
                    ]);
                }
            }
        }

        Ok(())
    }

    fn name(&self) -> &'static str {
        "GNOME"
    }
}

impl GnomeAdapter {
    fn apply_virtual_output(
        &self,
        _v: &crate::layout::VirtualOutput,
    ) -> Result<(), Box<dyn std::error::Error>> {
        warn!("Virtual output not fully supported on GNOME via gnome-randr");
        Ok(())
    }
}

fn gnome_randr(args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("gnome-randr")
        .args(args)
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            info!("gnome-randr {}: {}", args.join(" "), stdout.trim());
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "gnome-randr {} failed: {}",
            args.join(" "),
            stderr.trim()
        );
    }

    Ok(())
}
