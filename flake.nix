{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  outputs =
    inputs@{ flake-parts, nixpkgs, fenix, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];

      perSystem =
        {
          pkgs,
          inputs',
          ...
        }:
        let
          rust-toolchain = inputs'.fenix.packages.stable.toolchain;
        in
        {
          packages.default = pkgs.rustPlatform.buildRustPackage {
            pname = "madori";
            version = "0.1.0";
            src = ./.;
            cargoLock.lockFile = ./Cargo.lock;
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.systemd ];
          };

          devShells.default = pkgs.mkShell {
            packages = [
              rust-toolchain
              pkgs.pkg-config
              pkgs.systemd
            ];
          };
        };

      flake = {
        nixosModules.default = ./nix/module.nix;
      };
    };
}
