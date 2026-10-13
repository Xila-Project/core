# Releasing Xila

Xila uses [release-plz](https://release-plz.dev/) with reviewed Release PRs,
similar to getargs-derive. Releases are **GitHub-only**: crates remain unpublished
and native/WASM example archives are attached to versioned GitHub releases.
There are no rolling releases, `main-latest` updates, or crates.io tokens.
Crates.io readiness is tracked separately in [#282](https://github.com/xila-project/core/issues/282).
This replaces the rolling-release proposal in PR #274; the rolling requirements
originally described in #276 are intentionally excluded.

## Versions and changelog

All 42 packages in the root Cargo workspace inherit `[workspace.package].version`
and `publish = false`. The standalone WASM translation-test workspace has its own
unpublished manifest and is not part of release versioning. CI rejects members
that stop inheriting the workspace policy. Only the root `xila` package receives
a `vX.Y.Z` tag and GitHub release; its changelog includes the other packages.
All packages belong to release-plz's `xila` version group so changes confined to
an internal crate still contribute to the shared version bump. Internal
package tags, GitHub releases and separate changelogs are disabled.

The initial baseline is **0.1.0**, not a fabricated historical tag. Review the
first generated Release PR and its initial changelog before merging it.
`CHANGELOG.md` entries cover changes since the preceding version tag and become
the corresponding GitHub release notes. Rolling tags are excluded from analysis.
The lockfile remains ignored under the existing repository policy.

Squash-merge PRs so their Conventional Commit titles become commit messages.
The `PR Title` check validates titles such as `fix: correct a driver`,
`feat: add an application`, or `feat!: change the ABI`. Mark breaking changes
with `!` or a `BREAKING CHANGE:` footer in the squash commit.

| Commit | Before 1.0 | From 1.0 |
| --- | --- | --- |
| `fix:` and other nonbreaking changes | patch | patch |
| `feat:` | patch | minor |
| Breaking changes | minor | major |

release-plz proposes the bump from commit metadata; maintainers have the final
say and may edit the shared version and changelog in the Release PR. API-level
`cargo-semver-checks` is disabled for this Git-only, embedded workspace; review
ABI/API compatibility explicitly. Older nonconventional commits remain in the
initial changelog under Other.

## Flow

1. Ordinary merges to `main` open/update a Release PR with version and changelog
   changes. They do not build downloadable artifacts or publish a release.
2. Review the Release PR and its CI results. Merge it when a release is wanted.
3. The serialized `Release` workflow validates the exact merged source SHA,
   version and changelog, then builds English-locale x86_64 Linux native and
   browser WASM examples using the pinned Devbox tools.
4. After successful builds, release-plz creates the matching tag and a **draft**
   GitHub release. The workflow uploads every required asset, downloads them
   again to verify SHA-256 checksums/source metadata, and publishes the draft
   only as the final step.

The publication gate is the release workflow's policy/build/package checks;
the separate host/guest `Check` workflow and branch protection still apply to
PR review. Tags and artifacts are pinned to the initiating SHA, not the latest
tip of `main`. Tag-created workflows are not required: publication continues
in the same run, including with GitHub's default token.

## Downloaded artifacts

- `xila-native-linux-x86_64.tar.gz`: native example, calculator/weather WASI
  applications in `executables/`, and usage instructions. Extract the whole
  archive; run `./native_example --help` or
  `./native_example --terminal --no-network` from a writable directory.
  Cargo is not needed for the bundle. Linux/glibc and applicable graphics
  system libraries are required; it is not a portable static executable.
- `xila-wasm-demo-web.tar.gz`: the Trunk `dist/` directory, including WASM,
  JavaScript, HTML and browser assets. Extract and serve over localhost or
  HTTPS, for example `python3 -m http.server --directory dist 8000`.
  Do not open the HTML with a `file://` URL; browser APIs require a secure
  context. Browser disk data persists in OPFS as in the normal demonstration.
- `BUILD_INFO.txt`: workspace version, full source SHA and workflow URL.
- `SHA256SUMS`: checksums for both archives and `BUILD_INFO.txt`.

Download all four files to one directory and run `sha256sum --check SHA256SUMS`
before extracting. The native bundle lookup uses the executable's directory,
not the working directory. An incomplete bundle is an error; source builds
without a bundle keep the existing runtime Cargo-build behavior.

## Required repository setup

- Enable Actions to create pull requests under Settings → Actions → General.
- For automatic CI on bot-created Release PRs, set `RELEASE_PLZ_TOKEN` to a
  fine-grained PAT with Contents and Pull requests read/write access to this
  repository (or adapt the PR job to use a GitHub App token). The workflow
  falls back to `GITHUB_TOKEN` if the secret is absent, matching getargs-derive.
  PRs opened with that fallback do **not** automatically trigger PR workflows:
  a maintainer must close and reopen the Release PR before merging it.
- Use squash merges and require `Release policy` and `Conventional Commit title`
  along with the existing checks. Review generated version/changelog changes.
- Permit the publishing job to create version tags and draft releases under
  any repository rulesets. Write permissions are limited to publication and
  Release PR preparation; artifact building uses read-only permissions.

## Failures and retrying

Build/package/changelog failures do not create a public release. Upload or
verification failures leave a draft and preserve the last published version.
Already published versioned releases are never overwritten or clobbered.

Rerun **all jobs of the original workflow run** to retry its exact SHA. A
matching draft (or matching tag whose draft creation previously failed) can
be resumed. Clobbering assets is allowed only while that version is a draft;
all uploads must be verified before publication. Manual workflow dispatch
on `main` can retry only the current SHA. If `main` has advanced beyond a
pending draft, a retry there fails with instructions to rerun the original
run instead. Never move a version tag to make a retry pass.

Use `python3 development/release.py check-version` to check policy locally,
and `python3 -m unittest discover -s development/tests -p 'test_release.py' -v`
to exercise validation, packaging and mocked GitHub publication without
creating actual tags/releases.
