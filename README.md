# WlxOverlay X

A lightweight OpenXR overlay for Wayland desktops, inspired by XSOverlay.

This is a rewrite of the original [WlxOverlay for OpenVR](https://github.com/galister/WlxOverlay).

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

Will upload the rest of the info later.

