use crate::adapter;
use crate::config::Config;
use crate::detect;
use crate::layout;
use crate::matcher;
use log::{debug, info, warn};
use std::time::{Duration, SystemTime};

const CONFIG_PATH: &str = "/etc/madori/config.json";
const DEBOUNCE_MS: u64 = 300;

pub fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    info!("madori daemon starting");

    let mut config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to load config: {}", e);
            return Err(e);
        }
    };
    let mut config_mtime = get_config_mtime();

    // Apply initial layout
    if let Err(e) = apply_layout(&config) {
        warn!("Initial layout application failed: {}", e);
    }

    // Set up udev monitor
    let monitor = udev::MonitorBuilder::new()?.match_subsystem("drm")?.listen()?;

    let mut last_event: Option<std::time::Instant> = None;

    loop {
        // Check for config file changes
        let new_mtime = get_config_mtime();
        if new_mtime != config_mtime {
            info!("Config file changed, reloading");
            match load_config() {
                Ok(new_config) => {
                    config = new_config;
                    config_mtime = new_mtime;
                    if let Err(e) = apply_layout(&config) {
                        warn!("Layout re-application after config change failed: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Failed to reload config: {}", e);
                }
            }
        }

        // Drain all available udev events (non-blocking)
        let mut got_event = false;
        for event in monitor.iter() {
            let devtype = event
                .devtype()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "none".to_string());
            debug!(
                "udev event: {} {} action={:?}",
                event
                    .subsystem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "?".to_string()),
                devtype,
                event.event_type(),
            );
            got_event = true;
        }

        if got_event {
            last_event = Some(std::time::Instant::now());
        }

        // Check debounce
        if let Some(t) = last_event {
            if t.elapsed() >= Duration::from_millis(DEBOUNCE_MS) {
                last_event = None;
                info!("Applying layout due to display change");
                if let Err(e) = apply_layout(&config) {
                    warn!("Layout application failed: {}", e);
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    Config::load(CONFIG_PATH)
}

fn get_config_mtime() -> Option<SystemTime> {
    std::fs::metadata(CONFIG_PATH).ok().and_then(|m| m.modified().ok())
}

pub fn apply_once(config_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path.unwrap_or(CONFIG_PATH);
    let config = Config::load(path)?;
    apply_layout(&config)
}

pub fn apply_dry_run(config_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path.unwrap_or(CONFIG_PATH);
    let config = Config::load(path)?;

    let connectors = detect::detect_connectors()?;
    let matched = matcher::match_monitors(&config.monitors, &connectors);
    let maybe_rule = matcher::match_rules(&config.rules, &matched, &connectors);

    let de = adapter::detect_current_de();
    let adapter_name = adapter::create_adapter(&de)
        .map(|a| a.name().to_string());

    let resolved = maybe_rule.as_ref().map(|(_, rule)| {
        let wildcards = matcher::resolve_wildcards(rule, &matched, &connectors);
        layout::resolve_layout(&config, rule, &matched, &connectors, &wildcards)
    });

    let dry_run = DryRunOutput {
        desktop_environment: format!("{:?}", de),
        adapter: adapter_name.as_deref(),
        matched_rule_index: maybe_rule.as_ref().map(|(i, _)| *i),
        would_apply: maybe_rule.is_some(),
        resolved_layout: resolved.as_ref(),
    };

    let json = serde_json::to_string_pretty(&dry_run)?;
    println!("{}", json);
    Ok(())
}

#[derive(serde::Serialize)]
struct DryRunOutput<'a> {
    desktop_environment: String,
    adapter: Option<&'a str>,
    matched_rule_index: Option<usize>,
    would_apply: bool,
    resolved_layout: Option<&'a layout::ResolvedLayout>,
}

pub fn dump_state(config_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path.unwrap_or(CONFIG_PATH);
    let config = Config::load(path)?;
    let connectors = detect::detect_connectors()?;
    let matched = matcher::match_monitors(&config.monitors, &connectors);
    let maybe_rule = matcher::match_rules(&config.rules, &matched, &connectors);

    let dump = DumpOutput {
        connectors: &connectors,
        matched_monitors: &matched,
        matched_rule_index: maybe_rule.as_ref().map(|(i, _)| *i),
        matched_rule: maybe_rule.as_ref().map(|(_, r)| r),
    };

    let json = serde_json::to_string_pretty(&dump)?;
    println!("{}", json);
    Ok(())
}

#[derive(serde::Serialize)]
struct DumpOutput<'a> {
    connectors: &'a [detect::ConnectorInfo],
    matched_monitors: &'a std::collections::HashMap<String, String>,
    matched_rule_index: Option<usize>,
    matched_rule: Option<&'a crate::config::Rule>,
}

fn apply_layout(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let connectors = detect::detect_connectors()?;
    debug!("Detected {} connectors", connectors.len());

    let matched = matcher::match_monitors(&config.monitors, &connectors);
    debug!("Matched {} monitors", matched.len());

    let maybe_rule = matcher::match_rules(&config.rules, &matched, &connectors);
    let (_rule_index, rule) = match maybe_rule {
        Some(r) => {
            info!("Matched rule #{}", r.0);
            r
        }
        None => {
            info!("No matching rule found; keeping current layout");
            return Ok(());
        }
    };

    let wildcards = matcher::resolve_wildcards(&rule, &matched, &connectors);

    let resolved = layout::resolve_layout(config, &rule, &matched, &connectors, &wildcards);
    debug!("Resolved layout: {:?}", resolved);

    let de = adapter::detect_current_de();
    info!("Detected desktop environment: {:?}", de);

    if let Some(adapter) = adapter::create_adapter(&de) {
        info!("Applying layout via {}", adapter.name());
        adapter.apply(&resolved)?;
        info!("Layout applied successfully");
    } else {
        warn!("No adapter available for desktop environment {:?}", de);
    }

    Ok(())
}
