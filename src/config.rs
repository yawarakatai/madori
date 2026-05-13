use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub monitors: HashMap<String, MonitorDefinition>,
    pub rules: Vec<Rule>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MonitorDefinition {
    #[serde(rename = "matchBy")]
    pub match_by: MatchBy,
    #[serde(default)]
    pub scale: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MatchBy {
    #[serde(default)]
    pub connector: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub vendor: Option<String>,
    #[serde(default)]
    pub serial: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Rule {
    #[serde(rename = "match")]
    pub match_patterns: Vec<String>,
    #[serde(default)]
    pub layout: Option<HashMap<String, LayoutSpec>>,
    #[serde(default, rename = "virtual")]
    pub virtual_output: Option<VirtualSpec>,
    #[serde(default)]
    pub pre_hook: Option<String>,
    #[serde(default)]
    pub post_hook: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LayoutSpec {
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub transform: Option<String>,
    #[serde(default)]
    pub mirror: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VirtualSpec {
    pub width: u32,
    pub height: u32,
    #[serde(default = "default_refresh")]
    pub refresh: f64,
}

fn default_refresh() -> f64 {
    60.0
}

impl Config {
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let contents = std::fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&contents)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let json = r#"{
            "monitors": {},
            "rules": []
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert!(config.monitors.is_empty());
        assert!(config.rules.is_empty());
    }

    #[test]
    fn parse_monitor_full() {
        let json = r#"{
            "monitors": {
                "ally": {
                    "matchBy": {
                        "connector": "eDP-1",
                        "model": "B140HAN",
                        "vendor": "BOE",
                        "serial": "0x00000001"
                    },
                    "scale": 1.5
                }
            },
            "rules": []
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let m = &config.monitors["ally"];
        assert_eq!(m.match_by.connector.as_deref(), Some("eDP-1"));
        assert_eq!(m.match_by.model.as_deref(), Some("B140HAN"));
        assert_eq!(m.match_by.vendor.as_deref(), Some("BOE"));
        assert_eq!(m.match_by.serial.as_deref(), Some("0x00000001"));
        assert_eq!(m.scale, Some(1.5));
    }

    #[test]
    fn parse_monitor_minimal() {
        let json = r#"{
            "monitors": {
                "ally": { "matchBy": {} }
            },
            "rules": []
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let m = &config.monitors["ally"];
        assert!(m.match_by.connector.is_none());
        assert!(m.match_by.model.is_none());
        assert!(m.match_by.vendor.is_none());
        assert!(m.match_by.serial.is_none());
        assert!(m.scale.is_none());
    }

    #[test]
    fn parse_rule_with_layout_and_virtual() {
        let json = r#"{
            "monitors": {},
            "rules": [
                {
                    "match": ["ally", "innocn"],
                    "layout": {
                        "ally": { "position": "0,0", "scale": 1.5 },
                        "innocn": { "position": "auto,0", "scale": 1.0 }
                    }
                },
                {
                    "match": ["*"],
                    "virtual": { "width": 1920, "height": 1080, "refresh": 60 }
                }
            ]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules.len(), 2);
        assert_eq!(config.rules[0].match_patterns, vec!["ally", "innocn"]);
        let layout = config.rules[0].layout.as_ref().unwrap();
        assert_eq!(layout["ally"].scale, Some(1.5));
        let v = config.rules[1].virtual_output.as_ref().unwrap();
        assert_eq!(v.width, 1920);
        assert_eq!(v.height, 1080);
        assert_eq!(v.refresh, 60.0);
    }

    #[test]
    fn parse_virtual_default_refresh() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["*"], "virtual": { "width": 1280, "height": 720 } }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let v = config.rules[0].virtual_output.as_ref().unwrap();
        assert_eq!(v.refresh, 60.0);
    }

    #[test]
    fn parse_layout_with_mirror() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["ally", "_"], "layout": { "ally": { "position": "0,0" }, "$1": { "mirror": "ally" } } }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let layout = config.rules[0].layout.as_ref().unwrap();
        assert_eq!(layout["$1"].mirror.as_deref(), Some("ally"));
    }

    #[test]
    fn parse_layout_with_disabled() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["innocn"], "layout": { "innocn": { "position": "0,0", "enabled": false } } }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let layout = config.rules[0].layout.as_ref().unwrap();
        assert_eq!(layout["innocn"].enabled, Some(false));
    }

    #[test]
    fn parse_layout_enabled_defaults_to_none() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["ally"], "layout": { "ally": { "position": "0,0" } } }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let layout = config.rules[0].layout.as_ref().unwrap();
        assert_eq!(layout["ally"].enabled, None);
    }

    #[test]
    fn parse_layout_with_mode() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["innocn"], "layout": { "innocn": { "position": "0,0", "mode": "3840x2160@144" } } }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let layout = config.rules[0].layout.as_ref().unwrap();
        assert_eq!(layout["innocn"].mode.as_deref(), Some("3840x2160@144"));
    }

    #[test]
    fn parse_layout_with_transform() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["ally"], "layout": { "ally": { "position": "0,0", "transform": "right" } } }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let layout = config.rules[0].layout.as_ref().unwrap();
        assert_eq!(layout["ally"].transform.as_deref(), Some("right"));
    }

    #[test]
    fn parse_rule_with_hooks() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["ally"], "layout": { "ally": { "position": "0,0" } }, "pre_hook": "echo hello", "post_hook": "echo done" }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        let rule = &config.rules[0];
        assert_eq!(rule.pre_hook.as_deref(), Some("echo hello"));
        assert_eq!(rule.post_hook.as_deref(), Some("echo done"));
    }

    #[test]
    fn parse_hooks_default_to_none() {
        let json = r#"{
            "monitors": {},
            "rules": [{ "match": ["ally"], "layout": { "ally": { "position": "0,0" } } }]
        }"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.rules[0].pre_hook, None);
        assert_eq!(config.rules[0].post_hook, None);
    }
}
