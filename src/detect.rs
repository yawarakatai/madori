use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorInfo {
    pub name: String,
    pub connected: bool,
    pub edid: Option<EdidInfo>,
    pub modes: Vec<VideoMode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdidInfo {
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub serial: Option<String>,
    pub display_size_mm: Option<(u32, u32)>,
    pub preferred_mode: Option<VideoMode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoMode {
    pub width: u32,
    pub height: u32,
    pub refresh: f64,
}

fn read_file(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_binary(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

pub fn detect_connectors() -> Result<Vec<ConnectorInfo>, Box<dyn std::error::Error>> {
    let mut connectors = Vec::new();
    let drm_path = Path::new("/sys/class/drm");

    if !drm_path.exists() {
        return Ok(connectors);
    }

    let entries = std::fs::read_dir(drm_path)?;

    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();

        // Match cardX-outputname pattern (skip cardX and renderD*)
        if !name.contains('-') {
            continue;
        }

        // Strip "cardX-" prefix to get clean connector name (e.g., "card1-HDMI-A-1" -> "HDMI-A-1")
        let clean_name = if name.starts_with("card") {
            if let Some(pos) = name.find('-') {
                name[pos + 1..].to_string()
            } else {
                name.clone()
            }
        } else {
            name.clone()
        };

        let status = read_file(&entry.path().join("status"));

        let connected = matches!(status.as_deref(), Some("connected"));

        let edid = if connected {
            let edid_path = entry.path().join("edid");
            let raw = read_binary(&edid_path);
            raw.and_then(|data| parse_edid(&data))
        } else {
            None
        };

        let modes = if connected {
            let modes_path = entry.path().join("modes");
            read_modes(&modes_path)
        } else {
            Vec::new()
        };

        connectors.push(ConnectorInfo {
            name: clean_name,
            connected,
            edid,
            modes,
        });
    }

    Ok(connectors)
}

fn parse_edid(data: &[u8]) -> Option<EdidInfo> {
    if data.len() < 128 {
        return None;
    }

    // Check EDID magic header: 00 FF FF FF FF FF FF 00
    if data[0..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }

    // Manufacturer ID: bytes 8-9, compressed ASCII (5 bits per char)
    let mfg_high = data[8] as u16;
    let mfg_low = data[9] as u16;
    let mfg = ((mfg_high << 8) | mfg_low) >> 0;
    let vendor = {
        let c1 = (((mfg >> 10) & 0x1F) as u8) + b'A' - 1;
        let c2 = (((mfg >> 5) & 0x1F) as u8) + b'A' - 1;
        let c3 = ((mfg & 0x1F) as u8) + b'A' - 1;
        Some(format!("{}{}{}", c1 as char, c2 as char, c3 as char))
    };

    // Serial number: bytes 12-15 (little-endian)
    let serial = if data.len() >= 16 {
        let sno = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        if sno != 0 {
            Some(format!("{:08}", sno))
        } else {
            None
        }
    } else {
        None
    };

    // Display size in mm: bytes 21 (width), 22 (height)
    let display_size_mm = if data.len() >= 23 {
        let w = data[21] as u32;
        let h = data[22] as u32;
        if w > 0 || h > 0 {
            Some((w, h))
        } else {
            None
        }
    } else {
        None
    };

    // Descriptor blocks: bytes 54-125 (18 bytes each, 4 blocks)
    let mut model = None;
    let mut preferred_mode = None;

    for block_start in (54..=108).step_by(18) {
        if block_start + 18 > data.len() {
            break;
        }

        // Check if this is a detailed timing descriptor (non-zero pixel clock)
        let pixel_clock = u16::from_le_bytes([data[block_start], data[block_start + 1]]);
        if pixel_clock > 0 {
            // Detailed timing descriptor
            if preferred_mode.is_none() {
                let ha = (data[block_start + 2] as u32) | (((data[block_start + 4] as u32) >> 4) << 8);
                let hbl = (data[block_start + 3] as u32) | (((data[block_start + 4] as u32) & 0x0F) << 8);
                let va = (data[block_start + 5] as u32) | (((data[block_start + 7] as u32) >> 4) << 8);
                let vbl = (data[block_start + 6] as u32) | (((data[block_start + 7] as u32) & 0x0F) << 8);
                let h_total = ha + hbl;
                let v_total = va + vbl;
                if h_total > 0 && v_total > 0 {
                    let refresh = (pixel_clock as f64 * 10_000.0) / (h_total as f64 * v_total as f64);
                    preferred_mode = Some(VideoMode {
                        width: ha,
                        height: va,
                        refresh,
                    });
                }
            }
            continue;
        }

        let tag = data[block_start + 3];
        match tag {
            0xFC => {
                // Monitor name descriptor
                let text: Vec<u8> = data[block_start + 5..block_start + 18]
                    .iter()
                    .copied()
                    .take_while(|&b| b != 0x0A && b != 0x00)
                    .collect();
                if !text.is_empty() {
                    model = Some(String::from_utf8_lossy(&text).trim().to_string());
                }
            }
            0xFF => {
                // Monitor serial number descriptor
                let text: Vec<u8> = data[block_start + 5..block_start + 18]
                    .iter()
                    .copied()
                    .take_while(|&b| b != 0x0A && b != 0x00)
                    .collect();
                if !text.is_empty() {
                    let s = String::from_utf8_lossy(&text).trim().to_string();
                    if !s.is_empty() {
                        // Override serial with descriptor if available
                        // We keep the numeric serial as primary
                    }
                }
            }
            _ => {}
        }
    }

    Some(EdidInfo {
        model,
        vendor,
        serial,
        display_size_mm,
        preferred_mode,
    })
}

fn read_modes(path: &Path) -> Vec<VideoMode> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut modes = Vec::new();
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some((w, h)) = line.split_once('x') {
            if let (Ok(width), Ok(height)) = (w.parse::<u32>(), h.parse::<u32>()) {
                modes.push(VideoMode {
                    width,
                    height,
                    refresh: 60.0, // Default; will be updated from EDID
                });
            }
        }
    }
    modes
}
