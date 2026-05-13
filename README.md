# madori (間取り)

Intelligent display layout manager for Wayland compositors. Bridges declarative configuration with dynamic hardware changes (display hotplug).

```
madori daemon    # udev-watching daemon
madori apply     # detect current state, apply once
madori dump      # current state + match result as JSON
madori detect    # raw EDID/connector info (debug)
```

Supports niri, GNOME, Hyprland, and KDE. Runs on any Linux distribution with a `/etc/madori/config.json`.

## Configuration

madori reads `/etc/madori/config.json`. On NixOS this is generated automatically via the NixOS module; on other distributions create it manually.

### Monitor definitions

```json
{
  "monitors": {
    "ally": {
      "matchBy": { "connector": "eDP-1" },
      "scale": 1.5
    },
    "innocn": {
      "matchBy": { "connector": "HDMI-A-1", "model": "32M2V" },
      "scale": 1.0
    },
    "projector": {
      "matchBy": { "vendor": "EPSON" },
      "scale": 1.0
    }
  }
}
```

`matchBy` fields (all optional, AND-combined):

| Field        | Example     | Source                  |
|-------------|-------------|--------------------------|
| `connector` | `"eDP-1"`   | `/sys/class/drm` name    |
| `model`     | `"32M2V"`   | EDID Monitor Name        |
| `vendor`    | `"BOE"`     | EDID Manufacturer ID     |
| `serial`    | `"0x..."`   | EDID Serial Number       |

### Rules and pattern matching

Rules are evaluated top-to-bottom. The first matching rule is applied. If no rule matches, the current layout is preserved.

| Pattern | Meaning |
|---------|---------|
| `"ally"` | Named monitor is connected |
| `"_"`    | One unknown monitor (wildcard) |
| `"$1"`   | References the 1st wildcard in the layout |
| `"*"`    | Catch-all (0+ monitors, any state) |

#### Laptop only

```json
{
  "match": ["ally"],
  "layout": {
    "ally": { "position": "0,0" }
  }
}
```

#### Laptop + external (auto position)

`auto` packs monitors left-to-right on the same row:

```json
{
  "match": ["ally", "innocn"],
  "layout": {
    "ally":   { "position": "0,0" },
    "innocn": { "position": "auto,0", "scale": 1.0 }
  }
}
```

#### Mirror any external display

`_` matches one unknown monitor, `$1` references it in the layout:

```json
{
  "match": ["ally", "_"],
  "layout": {
    "ally": { "position": "0,0" },
    "$1":   { "mirror": "ally" }
  }
}
```

#### Headless fallback (virtual output)

`*` catches any remaining state. Useful when niri has no physical output:

```json
{
  "match": ["*"],
  "virtual": { "width": 1920, "height": 1080, "refresh": 60 }
}
```

### Layout fields

| Field       | Type             | Example            | Notes                              |
|-------------|------------------|--------------------|-------------------------------------|
| `position`  | `"x,y"` or `"auto,y"` | `"0,0"`, `"auto,0"` | required                         |
| `scale`     | float            | `1.5`              | inherits from monitor definition    |
| `transform` | string           | `"left"`, `"right"`, `"inverted"` | |
| `mirror`    | monitor name     | `"ally"`           | clones another output               |

## NixOS Integration

```nix
services.madori = {
  enable = true;
  monitors = {
    ally = {
      matchBy.connector = "eDP-1";
      scale = 1.5;
    };
    innocn = {
      matchBy.connector = "HDMI-A-1";
      matchBy.model = "32M2V";
    };
  };
  rules = [
    {
      match = [ "ally" "innocn" ];
      layout = {
        ally.position = "0,0";
        innocn.position = "auto,0";
      };
    }
  ];
};
```

A systemd user service (`madori daemon`) is started with the graphical session.

## License

MIT
