{
  lib,
  rustPlatform,
  alsa-lib,
  cmake,
  cpm-cmake,
  fontconfig,
  freetype,
  libglvnd,
  libxkbcommon,
  mesa,
  openxr-loader,
  pipewire,
  pkg-config,
  wayland,
  xorg,
  # flake
  self,
  version,
}:
rustPlatform.buildRustPackage {
  pname = "wlx-overlay-x";
  inherit version;

  src = lib.cleanSource self;

  cargoLock = {
    lockFile = ../Cargo.lock;
    allowBuiltinFetchGit = true;
  };

  nativeBuildInputs = [
    cmake
    pkg-config
    rustPlatform.bindgenHook
  ];

  buildInputs = [
    alsa-lib
    fontconfig
    freetype
    libglvnd
    libxkbcommon
    mesa
    openxr-loader
    pipewire
    wayland
    xorg.libX11
  ];

  # From https://github.com/StardustXR/server/blob/0dc5b1a92f5707efa16c251602935bdfc47ee7f0/nix/stardust-xr-server.nix
  CPM_SOURCE_CACHE = "./build";
  postPatch = ''
    sk=$(echo $cargoDepsCopy/stereokit-sys-*/StereoKit)
    mkdir -p $sk/build/cpm

    # This is not ideal, the original approach was to fetch the exact cmake
    # file version that was wanted from GitHub directly, but at least this way it comes from Nixpkgs.. so meh
    cp ${cpm-cmake}/share/cpm/CPM.cmake $sk/build/cpm/CPM_0.32.2.cmake
  '';

  meta = with lib; {
    description = "WlxOverlay for OpenXR, written in Rust";
    homepage = "https://github.com/galister/wlx-overlay-x";
    license = licenses.gpl3Only;
    platforms = platforms.linux;
    maintainers = with maintainers; [Scrumplex];
    mainProgram = "wlx-overlay-x";
  };
}
