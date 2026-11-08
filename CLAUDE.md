# Notes for agents

TileRider ported from C# / MonoGame (`suilevap/rokets`) to Rust, Bevy 0.20
and Avian 2D. Read `README.md` (running, options) and `ARCHITECTURE.md`
(modules, conventions, what is not ported) first. The migration plan is in
the C# repository: `docs/plans/RUST_BEVY_MIGRATION.md`.

- **Port intent, not implementation.** Find the C# code a feature comes
  from, work out what it was for (its git history goes back to 2009 and
  says why), and build what is natural in Bevy. Name the C# origin in the
  module's doc comment.
- The C# game is the reference: compare screenshots (`--screenshot`) and
  driving with it rather than copying its structure.
- Keep `content/`, `map/grid`, `light/bake` and `car/drive` free of Bevy
  app code so they stay unit-testable.
- Content comes from the C# repository via `tools/import_content.py`; do
  not edit files under `assets/` by hand.
- Documentation and code comments are in English.
- Toolchain: Rust 1.97.1 (`rust-toolchain.toml`). Run `cargo test` and
  `cargo clippy --all-targets` before committing.
