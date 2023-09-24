{
  description = "A lightweight OpenXR overlay for Wayland desktops, inspired by XSOverlay";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    self,
    flake-parts,
    ...
  }: let
    version = builtins.substring 0 8 self.lastModifiedDate or "dirty";
  in
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux"];
      perSystem = {
        config,
        pkgs,
        ...
      }: {
        packages = {
          default = config.packages.wlx-overlay-x;
          wlx-overlay-x = pkgs.callPackage ./nix/derivation.nix {inherit self version;};
        };

        formatter = pkgs.alejandra;
      };
    };
}
