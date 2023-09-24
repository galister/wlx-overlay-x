# WlxOverlay X

A lightweight OpenXR overlay for Wayland desktops, inspired by XSOverlay.

This is a rewrite of [WlxOverlay](https://github.com/galister/WlxOverlay) (That one is for OpenVR).

# Under Development

This project is in a highly experimental state. If you would like to give this a go, you might want to talk to me first.

- Discord: https://discord.gg/gHwJ2vwSWV
- Matrix Space: `#linux-vr-adventures:matrix.org`

# Usage

Recommend grabbing [rustup](https://rustup.rs/) if you don't have it yet.

Start Monado or any other OpenXR runtime. 

Required extensions are `EXTX_overlay` and `MND_egl_enable`.

```sh
cargo run
```

You'll see a screen and keyboard. You can turn these on and off using the watch on your left wrist.

Right click: turn your controller so that your backhand is facing your hmd. You'll get a yellow laser. Pull trigger for right-click.

Move screen: Grab using grip. Adjust distace using stick up/down while gripping.

Resize screen: Same as Move screen but turn your controller to get the yellow laser.

## Nix Flake

A Nix Flake is availabe as `github:galister/wlx-overlay-x`. Cached builds are available using [garnix](https://garnix.io/). See [garnix docs](https://garnix.io/docs/caching) to see how to utilize this binary cache.

# Known Issues

StereoKit fails to build with OpenXR version 1.0.29. You can downgrade your OpenXR syetem package to fix this.

# Reporting Issues

Make sure to:
- set `RUST_LOG=debug`
- use a debug build to see GL assertion messages

# Works Used
- [freesound](https://freesound.org/), CC0 sound effects (find the sounds by searching for their number)
- [StereoKit](https://stereokit.net/), MIT
- [SimulaVR TextShader](https://github.com/SimulaVR/Simula/blob/82256ba4c9c933e85f41c3e0aa429314d7275228/addons/godot-haskell-plugin/TextShader.tres), MIT - StereoKit port from [StardustXR](https://github.com/StardustXR/server/blob/main/src/wayland/shaders/shader_unlit_simula.sks), GPL-2
- See [Cargo.toml](https://github.com/galister/wlx-overlay-x/blob/main/Cargo.toml) for full list.
