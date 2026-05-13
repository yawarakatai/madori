use crate::adapter;
use crate::config::Config;
use crate::detect;
use crate::layout;
use crate::matcher;
use log::{debug, info, warn};
use std::time::{Duration, SystemTime};

const CONFIG_PATH: &str = "/etc/madori/config.json";

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
                let debounce = Duration::from_millis(config.debounce_ms);
                if t.elapsed() >= debounce {
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

pub fn show_status(config_path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path.unwrap_or(CONFIG_PATH);
    let config = Config::load(path)?;
    let connectors = detect::detect_connectors()?;
    let matched = matcher::match_monitors(&config.monitors, &connectors);
    let maybe_rule = matcher::match_rules(&config.rules, &matched, &connectors);

    println!("Display Status");
    println!("==============");

    // List connected displays
    let connected: Vec<_> = connectors.iter().filter(|c| c.connected).collect();
    if connected.is_empty() {
        println!("No displays connected");
    } else {
        for c in &connected {
            let edid_info = c.edid.as_ref();
            let model = edid_info.and_then(|e| e.model.as_deref()).unwrap_or("-");
            let vendor = edid_info.and_then(|e| e.vendor.as_deref()).unwrap_or("-");
            let preferred = edid_info.and_then(|e| e.preferred_mode.as_ref());

            let monitor_name = matched.get(&c.name).map(|s| s.as_str()).unwrap_or("-");

            println!("  {:<16} connected   model={:<16} vendor={:<6} matched={}",
                c.name, model, vendor, monitor_name);

            if let Some(m) = preferred {
                println!("    preferred mode: {}x{}@{:.0}Hz", m.width, m.height, m.refresh);
            }
            if !c.modes.is_empty() {
                let mut seen = std::collections::HashSet::new();
                let unique_modes: Vec<String> = c.modes.iter()
                    .filter_map(|m| {
                        let s = format!("{}x{}@{:.0}", m.width, m.height, m.refresh);
                        if seen.insert(s.clone()) { Some(s) } else { None }
                    })
                    .collect();
                println!("    available: {}", unique_modes.join(", "));
            }
        }
    }

    // Show matched rule
    println!();
    match maybe_rule {
        Some((idx, rule)) => {
            println!("Matched rule #{}", idx);
            if let Some(ref hooks) = rule.pre_hook {
                println!("  pre-hook:  {}", hooks);
            }
            println!("  patterns:  {:?}", rule.match_patterns);

            let wildcards = matcher::resolve_wildcards(&rule, &matched, &connectors);
            let resolved = layout::resolve_layout(&config, &rule, &matched, &connectors, &wildcards);

            if let Some(ref v) = resolved.virtual_output {
                println!("  virtual:   {}x{}@{:.0}Hz", v.width, v.height, v.refresh);
            }

            println!();
            println!("Resolved Layout");
            println!("---------------");
            for m in &resolved.monitors {
                let status = if m.enabled { "on " } else { "off" };
                let mirror = m.mirror.as_deref().unwrap_or("-");
                let mode_str = m.mode.as_ref()
                    .map(|mode| format!("{}x{}@{:.0}", mode.width, mode.height, mode.refresh))
                    .unwrap_or_else(|| "auto".to_string());
                println!(
                    "  {}  {:<16} pos={:>5},{:<5} scale={:<4} transform={:<8} mirror={:<6} mode={}",
                    status, m.monitor_name,
                    m.x, m.y,
                    m.scale,
                    m.transform,
                    mirror,
                    mode_str,
                );
            }

            if let Some(ref hooks) = rule.post_hook {
                println!();
                println!("  post-hook: {}", hooks);
            }
        }
        None => {
            println!("No rule matched. Current layout is preserved.");
        }
    }

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

    // Run pre-hook if defined
    if let Some(ref hook) = rule.pre_hook {
        info!("Running pre-hook: {}", hook);
        run_hook(hook);
    }

    if let Some(adapter) = adapter::create_adapter(&de) {
        info!("Applying layout via {}", adapter.name());
        adapter.apply(&resolved)?;
        info!("Layout applied successfully");
    } else {
        warn!("No adapter available for desktop environment {:?}", de);
    }

    // Run post-hook if defined
    if let Some(ref hook) = rule.post_hook {
        info!("Running post-hook: {}", hook);
        run_hook(hook);
    }

    Ok(())
}

fn run_hook(cmd: &str) {
    match std::process::Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .status()
    {
        Ok(status) if status.success() => {
            info!("Hook succeeded: {}", cmd);
        }
        Ok(status) => {
            warn!("Hook exited with {}: {}", status, cmd);
        }
        Err(e) => {
            warn!("Hook failed to run ({}): {}", e, cmd);
        }
    }
}
