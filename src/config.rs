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
