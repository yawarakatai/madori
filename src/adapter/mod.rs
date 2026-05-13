use crate::layout::ResolvedLayout;

pub mod niri;
pub mod gnome;
pub mod hyprland;
pub mod kde;

pub trait Adapter: Send + Sync {
    fn detect() -> Option<Box<dyn Adapter>>
    where
        Self: Sized;

    fn apply(&self, layout: &ResolvedLayout) -> Result<(), Box<dyn std::error::Error>>;

    fn name(&self) -> &'static str;
}

#[derive(Debug)]
pub enum DesktopEnvironment {
    Niri,
    Gnome,
    Hyprland,
    Kde,
    Unknown,
}

pub fn detect_current_de() -> DesktopEnvironment {
    // 1. Check $XDG_CURRENT_DESKTOP
    if let Ok(de) = std::env::var("XDG_CURRENT_DESKTOP") {
        let de_lower = de.to_lowercase();
        if de_lower.contains("niri") {
            return DesktopEnvironment::Niri;
        }
        if de_lower.contains("gnome") {
            return DesktopEnvironment::Gnome;
        }
        if de_lower.contains("hyprland") {
            return DesktopEnvironment::Hyprland;
        }
        if de_lower.contains("kde") {
            return DesktopEnvironment::Kde;
        }
    }

    // 2. Scan /proc for compositor binaries
    if let Ok(proc) = std::fs::read_dir("/proc") {
        for entry in proc.flatten() {
            let comm_path = entry.path().join("comm");
            if let Ok(comm) = std::fs::read_to_string(&comm_path) {
                let comm = comm.trim();
                match comm {
                    "niri" => return DesktopEnvironment::Niri,
                    "gnome-shell" => return DesktopEnvironment::Gnome,
                    "Hyprland" => return DesktopEnvironment::Hyprland,
                    "kwin_wayland" => return DesktopEnvironment::Kde,
                    _ => {}
                }
            }
        }
    }

    DesktopEnvironment::Unknown
}

pub fn create_adapter(de: &DesktopEnvironment) -> Option<Box<dyn Adapter>> {
    match de {
        DesktopEnvironment::Niri => niri::NiriAdapter::detect(),
        DesktopEnvironment::Gnome => gnome::GnomeAdapter::detect(),
        DesktopEnvironment::Hyprland => hyprland::HyprlandAdapter::detect(),
        DesktopEnvironment::Kde => kde::KdeAdapter::detect(),
        DesktopEnvironment::Unknown => None,
    }
}
