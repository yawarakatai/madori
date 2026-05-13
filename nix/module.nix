{ config, lib, pkgs, ... }:

let
  cfg = config.services.madori;
  inherit (lib) mkEnableOption mkOption types;
in
{
  options.services.madori = {
    enable = mkEnableOption "madori display layout daemon";

    package = mkOption {
      type = types.package;
      default = pkgs.madori or (throw "madori package not found. Set services.madori.package or add madori to pkgs.");
      description = "The madori package to use";
    };

    monitors = mkOption {
      type = types.attrs;
      default = { };
      description = "Monitor definitions (attrset of name -> { matchBy, scale? })";
    };

    rules = mkOption {
      type = types.listOf types.attrs;
      default = [ ];
      description = "Ordered list of match rules";
    };

    debounceMs = mkOption {
      type = types.int;
      default = 300;
      description = "Debounce time in milliseconds for udev events";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.etc."madori/config.json" = {
      text = builtins.toJSON {
        monitors = cfg.monitors;
        rules = cfg.rules;
        debounce_ms = cfg.debounceMs;
      };
      mode = "0444";
    };

    environment.systemPackages = [ cfg.package ];

    systemd.user.services.madori = {
      description = "madori display layout daemon";
      after = [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      wantedBy = [ "graphical-session.target" ];

      serviceConfig = {
        ExecStart = "${cfg.package}/bin/madori daemon";
        Restart = "on-failure";
        RestartSec = 5;
      };

      environment = {
        PATH = "%E/.nix-profile/bin:/etc/profiles/per-user/%u/bin:/run/current-system/sw/bin:/run/wrappers/bin";
        XDG_RUNTIME_DIR = "/run/user/%U";
      };
    };
  };
}
