{self, ...}: let
  version = builtins.substring 0 8 self.lastModifiedDate or "dirty";
in {
  perSystem = {
    config,
    pkgs,
    ...
  }: {
    packages = {
      default = config.packages.wlx-overlay-x;
      wlx-overlay-x = pkgs.callPackage ./derivation.nix {inherit self version;};
    };
  };

  flake.overlays.default = final: _: {
    wlx-overlay-x = final.callPackage ./derivation.nix {inherit self version;};
  };
}
