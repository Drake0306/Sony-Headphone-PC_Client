# Hairline

Hairline is a desktop client for Sony ULT WEAR (WH-ULT900N) headphones. It talks to the headphones directly over Bluetooth, without the official phone app, and puts the everyday controls in a small menu-bar popover.

## Status

Phase 1: a macOS menu-bar popover for the WH-ULT900N with battery level, noise control, ambient sound level and ULT level. Windows and Linux are planned.

## Run it

Prerequisites: Rust stable, Node 22, pnpm 10.

```
cd apps/desktop
pnpm install
pnpm tauri dev -- -- --fake-device
```

The double `--` is intentional: the Tauri CLI forwards the first one to cargo, and the second hands `--fake-device` to the app. Setting the environment variable also works:

```
HAIRLINE_FAKE=1 pnpm tauri dev
```

`--fake-device` runs the app against a simulated headphone instead of a real Bluetooth connection, so you can try the popover without the hardware.

## Tests

```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd apps/desktop && pnpm check
```

## Layout

- `crates/mdr-frame`: packet framing, escaping and checksums.
- `crates/mdr-proto`: message encoding and decoding, and the request deny-list.
- `crates/mdr-session`: handshake, acknowledgements and retries, and device state.
- `crates/mdr-transport`: the Bluetooth link to the headphones.
- `apps/desktop`: the Tauri 2 and Svelte 5 app (Rust side in `src-tauri`).

## Safety

Hairline never sends commands on the deny-list, and it only changes a setting the headphones declare as supported, outside the explicit Advanced panel flow. The details are in [docs/protocol-notes.md](docs/protocol-notes.md).

## Design

The approved interface is in [docs/research/reports/Hairline design screens.html](<docs/research/reports/Hairline design screens.html>). The full design is in [the spec](docs/superpowers/specs/2026-11-07-ult-wear-desktop-app-design.md).

## License

MIT. Third-party notices are in [THIRD_PARTY.md](THIRD_PARTY.md).

Not affiliated with or endorsed by Sony. Sony, ULT WEAR, DSEE and LDAC are trademarks of their owners.
