# How the book is published

The user guide is published automatically to GitHub Pages by the
[`Deploy book`](https://github.com/greysquirr3l/vortex-bevy/actions/workflows/book.yml)
workflow on every push to `main` that touches `book/`.

## Live site

The latest version of this guide is rendered at
<https://greysquirr3l.github.io/vortex-bevy/>. Every push to `main` that
changes `book/src/` triggers a rebuild and redeploy.

The site URL is set in [`book.toml`](../../book.toml) under
`[output.html].site-url = "/vortex-bevy/"`. GitHub Pages serves the
artifact at `<github-user>.github.io/<repo-name>/`, so the path prefix
matches the repository name.

## How the workflow works

The workflow is at
[`.github/workflows/book.yml`](https://github.com/greysquirr3l/vortex-bevy/blob/main/.github/workflows/book.yml).
On every relevant push it:

1. Installs mdbook 0.5.4 via `cargo install --locked --version 0.5.4`.
2. Runs `mdbook build book/` to produce the static HTML under
   `book/build/` (gitignored).
3. Runs `mdbook test book/` to catch broken intra-doc links and missing
   examples that a plain build would skip.
4. Configures the GitHub Pages environment.
5. Uploads `book/build/` as a Pages artifact.
6. Deploys the artifact to GitHub Pages via the official
   `actions/deploy-pages` action.

The workflow uses the official Pages-artifact deployment path rather
than the older `gh-pages` branch approach. There is no separate branch
to manage — the deployment state lives entirely in GitHub's Pages
infrastructure.

The Pages upload opts into hidden files. mdbook writes a `.nojekyll`
file into `book/build/` to tell GitHub Pages to serve the site without
running it through Jekyll, and `actions/upload-pages-artifact` stopped
shipping hidden files by default in v4. Without `include-hidden-files`,
that marker would be dropped from the artifact and the deployed site
would be processed by Jekyll.

## Triggers

The workflow runs on:

- `push` to `main` with paths under `book/**` or the workflow file itself.
- `workflow_dispatch` from the Actions tab, for manual deployments.

PRs to `main` do not trigger a deployment. PRs run the CI workflow
(`.github/workflows/ci.yml`), which already runs
`scripts/check-documentation.sh` and therefore builds and tests the
book as part of the quality gate.

## The workflow action contract

The CI quality job also runs `scripts/check-workflow-actions.sh`. That
script resolves the `action.yml` of every action the workflows pin and
checks two things for each step:

- Every key under `with:` names an input the action actually declares.
- Every required input the action declares is supplied.

`actionlint` covers workflow syntax, but it does not resolve remote
actions, so it cannot catch either mistake. A wrong `with:` key is
silently ignored by the action, and a missing required input fails the
job only after the runner has already started. Both shipped undetected
because the CI jobs had never actually run.

The check caches each fetched `action.yml` under the system temporary
directory, and `scripts/test-workflow-actions.sh` exercises the parsing
rules against synthetic manifests so the contract is covered without
network access.

## Required repository settings

The workflow depends on two repository-level settings that you
configure in `Settings > Pages`:

1. **Build and deployment** must be set to **GitHub Actions** (not
   "Deploy from a branch"). The workflow uses the official Pages
   artifact deploy path; the branch-based path is unused.
2. The `github-pages` environment must exist with the default
   permissions. The workflow grants the deployment job
   `pages: write` and `id-token: write` (for OIDC token exchange with
   the Pages service).

### Branch protection

`main` requires six status checks and one approving review. The
required checks are named after the CI job display names, not the job
ids:

| Required check | CI job |
|---|---|
| `Quality (ubuntu-latest)` | `quality` |
| `Quality (macos-14)` | `quality` |
| `Quality (windows-latest)` | `quality` |
| `Supply chain and fixture policy` | `supply-chain` |
| `Authorized Editor bridge quality` | `bridge-quality` |
| `Secret scan` | `secret-scan` |

GitHub Actions reports each job as a check run named after the job's
`name` field, so a required check that uses the job id instead of the
display name never matches a result. A check that never reports leaves
the pull request blocked permanently, even when every job passes. When
you rename a CI job, update the branch protection rule in the same
change.

The expected contexts are recorded in
[`.github/branch-protection.json`](../../../.github/branch-protection.json).
`scripts/check-branch-protection.sh` compares that record against the CI
workflow and runs in the CI quality job and the release readiness
script. Pass `--live` to also compare the record against the live rule,
which needs a token with admin access to the repository.

The rule is set to *strict*, so the checks must pass against the
current head of `main`. Rebase before expecting the checks to go
green.

### Disk budget

The GitHub-hosted runners have roughly 14 GB of free disk. A default
debug build of this workspace is 12 GB, and `cargo test` and `cargo l`
each add their own artifacts, so the runner runs out of space partway
through the link step.

The symptom is misleading. The runner reports `ld terminated with
signal 7 [Bus error]`, which points at the linker rather than at the
disk, and the build steps that follow are simply reported as skipped.

The CI workflow sets `CARGO_PROFILE_DEV_DEBUG=0`,
`CARGO_PROFILE_TEST_DEBUG=0`, and `CARGO_INCREMENTAL=0`, which brings
the build to 7.5 GB and makes it faster to link. The release bundle
workflow leaves the release profile alone, so the shipped binary keeps
its debug symbols; a release build is 3.2 GB and fits.

`scripts/check-build-disk.sh` measures the build directory against a
10 GB budget and runs after the build in both workflows. It fails with
a message that names the cause instead of leaving a bus error to be
interpreted.

### Headless GPU

The runtime tests build a Bevy app and tick it, which needs a graphics
adapter. The GitHub-hosted ubuntu runner ships the Vulkan loader but no
Vulkan ICD, so wgpu finds no adapter and Bevy panics with `Unable to
find a GPU`. The CI workflow installs `mesa-vulkan-drivers`, which
provides lavapipe, the software Vulkan renderer.

The macOS runner has a real GPU and needs nothing extra. The Windows
runner ships a software adapter that wgpu finds without help.

## Adding a chapter

To add a new chapter:

1. Write the chapter Markdown under `book/src/<section>/<name>.md`.
2. Add the chapter to `book/src/SUMMARY.md` in the appropriate section.
3. Open a PR. The CI workflow builds and tests the book; the link
   checker catches broken cross-references.
4. After merge, the `Deploy book` workflow rebuilds and publishes the
   updated guide.

## Local build

To build the book locally:

```bash
mdbook build book/
mdbook test book/
```

Or via the Justfile recipe:

```bash
just book
```

The output goes to `book/build/`. Open `book/build/index.html` in a
browser to preview.
