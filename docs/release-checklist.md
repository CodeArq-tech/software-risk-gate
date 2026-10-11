# Release Checklist

- [ ] Update the version in Cargo.toml, action.yml, and the changelog.
- [ ] Run cargo fmt, Clippy, and all tests.
- [ ] Confirm line coverage is above 95% in the workspace.
- [ ] Confirm mutation score is above 95%.
- [ ] Build the Linux binary with --release --locked.
- [ ] Generate a SHA-256 checksum for every release asset.
- [ ] Publish binaries and checksums under the same vX.Y.Z tag.
- [ ] Validate the Action in a test PR with PR-read and comment-write
      permissions.
- [ ] Confirm risk, status, merge_blocked, and report_url outputs are available
      to downstream jobs.
