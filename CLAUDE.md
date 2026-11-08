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
- **Tests live apart from the code they test**, never in the same file:
  - a test of public behaviour goes in `tests/` (an integration test);
  - a test that needs private items goes in its own file next to the
    module, `src/<module>/tests.rs` (or `src/<module>_tests.rs` for a
    single-file module), pulled in by one line in the module:
    `#[cfg(test)] mod tests;` (with `#[path = "<module>_tests.rs"]` where
    needed);
  - no `#[cfg(test)] mod tests { ... }` blocks inside a source file, and no
    test-only helpers in production code. Some existing modules still have
    inline tests; move them out when touching those modules.
- Documentation and code comments are in English.
- Toolchain: Rust 1.97.1 (`rust-toolchain.toml`). Run `cargo test` and
  `cargo clippy --all-targets` before committing.

## Web build and pull request previews

The game runs in the browser. `.github/workflows/pages.yml` builds it to
WebAssembly and publishes it to GitHub Pages from the `gh-pages` branch:
`main` at https://suilevap.github.io/rokets_rust/ and each pull request from
this repository at `https://suilevap.github.io/rokets_rust/pr/<number>/`
(built from the branch's own commit, about 10 minutes per push; removed
when the pull request closes). Previews run `main`'s copy of the workflow
(`pull_request_target`), so changes to `.github/workflows/pages.yml` or
`.github/scripts/` only affect previews once merged. The page's
bottom-right corner names the build it serves (`PR #<n> · <sha>`).

When a pull request changes anything a player could notice in the browser,
put the preview link in its description, pointed at what changed with URL
options (`map`, `start`, `drive`, e.g. `.../pr/42/?map=LetItSnowDay`), and
say what to try. Pull requests that only touch `**.md`, `docs/`, `tests/`,
`tools/` or `.claude/` build no preview; for other changes nobody needs to
play, add the `no-preview` label. Do not link a preview that does not exist.
