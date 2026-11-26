# Planet Simulator — contributor / agent guide

Rust + Bevy planetary evolution simulator. The project lives at the repository root (`Cargo.toml`, `src/`, `docs/`).

## Environment

- Stable Rust 1.97+ (built with 1.100). Bevy 0.20 requires ≥ 1.97.
- No Python or Node tooling is involved.
- Build artifacts go to `/target`; saves go to the platform data directory. Both are ignored.

## Verification

```bash
cargo check --lib          # fast type check of the Bevy-free core + app layer
cargo test                 # full test suite (generation, conservation, save/load, time)
cargo test <test_name>     # single test, e.g. cargo test water_budget_is_conserved
cargo run                  # launch the simulator
cargo fmt --check          # formatting
cargo clippy --all-targets # optional but recommended
```

First build compiles Bevy and can take several minutes. Tests use reduced icosphere levels (2–3), so they are fast after compilation.

## Architecture notes

- `src/lib.rs` exposes all modules; `src/main.rs` only boots Bevy.
- The simulation core (`planet_generator`, `terrain`, `planetary_physics`, `atmosphere`, `climate`, `hydrology`, `tectonics`, `weather`, `catastrophes`, `statistics`, `history`, `time_engine`, `simulation`, `save_system`, `experiments`) is Bevy-free and unit-tested.
- Bevy/egui layers: `app`, `rendering`, `ui`.
- Keep physical assumptions documented in `docs/MODEL.md` and status in `docs/STATUS.md`. Do not mark partial mechanics as implemented.
- Add a regression test for every numerical or conservation change.
- Headless diagnostic: `cargo run --profile test --example inspect -- 6`.
