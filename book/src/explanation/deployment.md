# How the book is published

The user guide is published automatically to GitHub Pages by the
[`Deploy book`](https://github.com/greysquirr3l/vortex-bevy/actions/workflows/book.yml)
workflow on every push to `main` that touches `book/`.

## Live site

The latest version of this guide is rendered at
<https://greysquirr3l.github.io/vortex-bevy/>. Every push to `main` that
changes `book/src/` triggers a rebuild and redeploy.

The site URL is set in [`book.toml`](../book.toml) under
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
