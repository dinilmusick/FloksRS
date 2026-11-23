# floksrs registry publishing

The checked-in Git revisions preserve the existing development/test
dependency pins. Corresponding `version` constraints now allow Cargo
to produce a crates.io-compatible manifest; they do **not** imply those
dependency versions have been published.

**Upstream order:** publish initial crates for KrnlRS, KloksRS and
FrnlRS (and FloksRS before SkipoRS). Their exact registry package
versions must exist and have independent consumer tests.

Set GitHub repository variable `CRATES_IO_DEPENDENCIES_READY=true`
only after that is verified; otherwise every release job is intentionally
skipped. On each push to main after enabling it, cargo tests and
`cargo publish --dry-run` must pass before release-plz can prepare
a version PR. Merging a release PR publishes to crates.io only after
initial manual bootstrap and crates.io Trusted Publisher registration.

For new crate names, the first publish cannot use OIDC: store a scoped
bootstrap token only in GitHub Actions secrets, trigger first publish
in GitHub Actions, register the `release-rust.yml` workflow and
`crates-io` environment with crates.io, then set
`CRATES_IO_TRUSTED_PUBLISHING_READY=true`. No local token is needed.
For fully automated release-PR merge with validation, a GitHub App
token and mandatory CI checks are still required.
