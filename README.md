# madori (間取り)

Intelligent display layout manager for Wayland compositors. Bridges declarative configuration with dynamic hardware changes (display hotplug).

```
madori daemon    # udev-watching daemon
madori apply     # detect current state, apply once
madori status    # human-readable display status
madori dump      # current state + match result as JSON
madori detect    # raw EDID/connector info (debug)
```

Supports **niri**, **GNOME**, **Hyprland**, and **KDE**. Runs on any Linux distribution.

## Quick Start

```json
// /etc/madori/config.json
{
  "monitors": {
    "ally":   { "matchBy": { "connector": "eDP-1" },                     "scale": 1.5 },
    "innocn": { "matchBy": { "connector": "HDMI-A-1", "model": "32M2V" }, "scale": 1.0 }
  },
  "rules": [
    {
      "match": ["ally", "innocn"],
      "layout": {
        "ally":   { "position": "0,0" },
        "innocn": { "position": "auto,0" }
      }
    },
    { "match": ["ally"], "layout": { "ally": { "position": "0,0" } } },
    { "match": ["*"],    "virtual": { "width": 1920, "height": 1080 } }
  ]
}
```

```bash
madori apply --dry-run   # preview without changing anything
madori apply             # apply once
madori daemon            # watch for display changes
```

## NixOS

```nix
services.madori = {
  enable = true;
  monitors = {
    ally.matchBy.connector = "eDP-1";
    ally.scale = 1.5;
    innocn = {
      matchBy.connector = "HDMI-A-1";
      matchBy.model = "32M2V";
    };
  };
  rules = [
    {
      match = [ "ally" "innocn" ];
      layout.ally.position = "0,0";
      layout.innocn.position = "auto,0";
    }
  ];
};
```

## Documentation

- **[CONFIG.md](CONFIG.md)** — Full configuration reference: pattern matching, layout fields, hooks, negation, examples

## License

MIT
