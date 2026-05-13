{ lib, ... }:
let
  inherit (lib) optionalAttrs filterAttrs mapAttrs;
in
rec {
  /* Create a monitor definition with type-safe matchBy fields and optional scale.

     Example:
       mkMonitor {
         connector = "eDP-1";
         scale = 1.5;
       }
       => { matchBy = { connector = "eDP-1"; }; scale = 1.5; }
  */
  mkMonitor =
    {
      connector ? null,
      model ? null,
      vendor ? null,
      serial ? null,
      scale ? null,
    }:
    {
      matchBy = filterAttrs (_: v: v != null) {
        inherit connector model vendor serial;
      };
    }
    // optionalAttrs (scale != null) { inherit scale; };

  /* Create a layout specification for a single monitor within a rule.

     Example:
       mkLayout {
         position = "0,0";
         scale = 1.5;
         transform = "right";
         mirror = "ally";
         enabled = false;
         mode = "1920x1080@60";
       }
  */
  mkLayout =
    {
      position ? null,
      scale ? null,
      transform ? null,
      mirror ? null,
      enabled ? null,
      mode ? null,
    }:
    filterAttrs (_: v: v != null) {
      inherit position scale transform mirror enabled mode;
    };

  /* Create a rule with pattern matching, layout, and optional virtual output.

     Example:
       mkRule {
         match = [ "ally" "innocn" ];
         layout = {
           ally = { position = "0,0"; scale = 1.5; };
           innocn = mkLayout { position = "auto,0"; };
         };
       }
  */
  mkRule =
    {
      match,
      layout ? { },
      virtual ? null,
      pre_hook ? null,
      post_hook ? null,
    }:
    {
      inherit match;
    }
    // optionalAttrs (layout != { }) {
      layout = mapAttrs (_: filterAttrs (_: v: v != null)) layout;
    }
    // optionalAttrs (virtual != null) { inherit virtual; }
    // optionalAttrs (pre_hook != null) { inherit pre_hook; }
    // optionalAttrs (post_hook != null) { inherit post_hook; };
}
