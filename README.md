# madori (間取り)

Intelligent display layout manager for NixOS. Bridges declarative Nix configuration with dynamic hardware changes (display hotplug).

```
madori daemon    # udev-watching daemon (systemd service)
madori apply     # detect current state, apply once
madori dump      # current state + match result as JSON
madori detect    # raw EDID/connector info (debug)
```

Supports niri, GNOME, Hyprland, and KDE Wayland compositors.

## NixOS Integration

```nix
services.madori = {
  enable = true;
  monitors = {
    ally.matchBy.connector = "eDP-1";
    innocn = {
      matchBy.connector = "HDMI-A-1";
      matchBy.model = "32M2V";
    };
  };
  rules = [
    { match = ["ally", "innocn"]; layout = { ally.position = "0,0"; innocn.position = "auto,0"; }; }
  ];
};
```

## License

MIT
