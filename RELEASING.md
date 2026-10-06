# Releasing

The three crates are released together, at the same version.

1. Pick the version. Until 1.0, raise the minor version for a release that
   breaks the API, and the patch version otherwise.
2. Set it as `version` under `[workspace.package]` in `Cargo.toml`, and in
   the two `=` requirements under `[workspace.dependencies]`. Run
   `cargo check` to update `Cargo.lock`.
3. In `CHANGELOG.md`, give the version's section the date of the release
   in place of "Unreleased".
4. Once CI passes on that commit, run `cargo publish --workspace --dry-run`,
   then `cargo publish --workspace`. Cargo publishes the core crate and the
   macros before the crate that depends on them.
5. Tag the commit `vX.Y.Z` and push the tag.
