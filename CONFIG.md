# madori Configuration Reference

## File location

`/etc/madori/config.json` (NixOS: auto-generated; other distros: create manually)

---

## Top-level fields

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `monitors` | object | required | Named monitor definitions |
| `rules` | array | required | Ordered list of matching rules |
| `debounce_ms` | integer | `300` | udev event debounce in milliseconds |

```json
{
  "monitors": { ... },
  "rules": [ ... ],
  "debounce_ms": 300
}
```

---

## Monitors

Each monitor has a user-chosen name and describes how to identify a physical display.

```json
{
  "monitors": {
    "ally": {
      "matchBy": { "connector": "eDP-1" },
      "scale": 1.5
    }
  }
}
```

### `matchBy` fields

All optional, AND-combined. At least one should be specified.

| Field | Type | Example | Source |
|-------|------|---------|--------|
| `connector` | string | `"eDP-1"` | `/sys/class/drm` connector name |
| `model` | string | `"32M2V"` | EDID Monitor Name descriptor (partial match) |
| `vendor` | string | `"BOE"` | EDID Manufacturer ID (3-char PNP, exact match) |
| `serial` | string | `"0x00000001"` | EDID Serial Number |

### `scale`

Default scale factor for this monitor. Can be overridden per-rule in `layout`.

---

## Rules

Rules are evaluated **top-to-bottom**. The first matching rule is applied. If no rule matches, the current layout is preserved.

```json
{
  "match": ["ally", "innocn"],
  "layout": {
    "ally":   { "position": "0,0", "scale": 1.5 },
    "innocn": { "position": "auto,0" }
  },
  "pre_hook": "killall swaybg || true",
  "post_hook": "swaybg -i ~/wallpaper.jpg -m fill &"
}
```

### Rule fields

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `match` | string[] | required | Pattern list |
| `layout` | object | `{}` | Per-monitor layout settings |
| `virtual` | object | `null` | Virtual output for headless fallback |
| `pre_hook` | string | `null` | Shell command before applying layout |
| `post_hook` | string | `null` | Shell command after applying layout |

### `virtual`

Creates a virtual (headless) output. Required for niri which cannot start without an output.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `width` | integer | required | Virtual output width |
| `height` | integer | required | Virtual output height |
| `refresh` | float | `60` | Refresh rate |

---

## Pattern matching syntax

| Pattern | Meaning | Consumes a connector? |
|---------|---------|------------------------|
| `"ally"` | Named monitor is connected | Yes |
| `"_"` | One unknown (unmatched) monitor | Yes |
| `"$1"` | References the 1st wildcard in layout | No (alias) |
| `"!innocn"` | Named monitor is **NOT** connected | No (filter) |
| `"*"` | Catch-all, matches any state | No |

### Pattern examples

**Laptop only** — ally is connected, nothing else:
```json
{ "match": ["ally"], "layout": { "ally": { "position": "0,0" } } }
```

**Laptop + any external, mirrored** — `_` captures the unknown display, `$1` names it:
```json
{
  "match": ["ally", "_"],
  "layout": {
    "ally": { "position": "0,0" },
    "$1":   { "mirror": "ally" }
  }
}
```

**Laptop only, but NOT when projector is plugged in** — `!name` filters the match:
```json
{
  "match": ["ally", "!projector"],
  "layout": { "ally": { "position": "0,0" } }
}
```

**Laptop + external + negation** — external is present but projector is not:
```json
{
  "match": ["ally", "_", "!projector"],
  "layout": {
    "ally": { "position": "0,0" },
    "$1":   { "position": "auto,0" }
  }
}
```

**Headless fallback** — matches any state not caught by earlier rules:
```json
{ "match": ["*"], "virtual": { "width": 1920, "height": 1080 } }
```

### Complete rule chain example

```json
"rules": [
  {
    "match": ["ally", "innocn"],
    "layout": {
      "ally":   { "position": "0,0",    "scale": 1.5 },
      "innocn": { "position": "auto,0", "scale": 1.0, "mode": "3840x2160@144" }
    }
  },
  {
    "match": ["ally", "_", "!innocn"],
    "layout": {
      "ally": { "position": "0,0" },
      "$1":   { "position": "auto,0" }
    }
  },
  {
    "match": ["ally", "!innocn"],
    "layout": { "ally": { "position": "0,0" } }
  },
  {
    "match": ["*"],
    "virtual": { "width": 1920, "height": 1080, "refresh": 60 }
  }
]
```

---

## Layout fields

Per-monitor settings inside a rule's `layout`:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `position` | string | — | `"x,y"` or `"auto,y"` (auto left-to-right pack) |
| `scale` | float | from monitor definition | Overrides monitor scale |
| `transform` | string | `"normal"` | `"left"`, `"right"`, `"inverted"`, `"normal"` |
| `mirror` | string | `null` | Monitor name to clone (niri: not supported) |
| `enabled` | bool | `true` | `false` to disable this output |
| `mode` | string | auto-detected | `"1920x1080@60"` or `"3840x2160@144"` |

### Auto positioning

`"auto,y"` places the monitor at the right edge of the previously placed monitor, on the same row. `y` controls the vertical offset.

```json
// ally at 0,0 (width 1920). innocn auto-placed at 1920,0. projector at 1920+3840,0.
{ "ally": { "position": "0,0" }, "innocn": { "position": "auto,0" }, "projector": { "position": "auto,0" } }
```

### Disabling outputs

```json
{ "innocn": { "position": "0,0", "enabled": false } }
```

Sends `off`/`disable` to the compositor instead of configuring the output.

### Explicit mode

```json
{ "innocn": { "position": "0,0", "mode": "2560x1440@144" } }
```

Overrides the auto-detected preferred mode. Format: `"WxH@R"` or `"WxH"` (defaults to 60Hz).

---

## Pre/post hooks

Shell commands run before and after layout application. Useful for wallpaper changes, notification daemon restarts, etc.

```json
{
  "match": ["ally", "innocn"],
  "pre_hook": "killall swaybg || true",
  "post_hook": "swaybg -i ~/wallpaper-ultrawide.jpg -m fill &",
  "layout": { ... }
}
```

- Non-zero exit codes are logged as warnings but do **not** prevent layout application.
- Hooks run in `/bin/sh -c`.

---

## CLI reference

```
madori daemon                Run as udev-watching daemon
madori apply                 Detect current state and apply layout once
madori apply --dry-run       Preview what would be applied (no changes)
madori status                Human-readable display and layout status
madori dump                  Current state + match result as JSON
madori detect                Raw EDID info for all connected outputs
```

All commands accept `-c <path>` to specify a custom config file path.

---

## NixOS module

```nix
services.madori = {
  enable = true;

  # Optional: override the package
  package = inputs.madori.packages.x86_64-linux.default;

  # udev debounce in ms
  debounceMs = 300;

  monitors = { ... };
  rules = [ ... ];
};
```

### Nix helper library

`inputs.madori.lib` provides type-safe helpers:

- `mkMonitor { connector = "eDP-1"; scale = 1.5; }`
- `mkLayout { position = "0,0"; scale = 1.5; }`
- `mkRule { match = [...]; layout = { ... }; }`

```nix
services.madori = {
  enable = true;
  monitors = {
    ally = inputs.madori.lib.mkMonitor {
      connector = "eDP-1";
      scale = 1.5;
    };
  };
  rules = [
    (inputs.madori.lib.mkRule {
      match = [ "ally" ];
      layout.ally = { position = "0,0"; };
    })
  ];
};
```

---

## Adapter behavior

| DE | Binary | Disable | Mirror | Virtual |
|----|--------|---------|--------|---------|
| niri | `niri` | `output off` | unsupported | `Virtual-1` |
| GNOME | `gnome-randr` | `--off` | `--same-as` | unsupported |
| Hyprland | `hyprctl` | `,disable` | `,mirror,` | `HEADLESS-1` |
| KDE | `kscreen-doctor` | `.disable` | `.replicate` | `HEADLESS-1` |

If the required binary is not found in `PATH`, the adapter logs a warning and continues without applying. This makes madori safe to run on machines where some compositors' tools are not installed.
