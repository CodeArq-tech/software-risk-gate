# Risk Gate marketing site

This static, responsive landing page lives in `site/` and supports English,
Portuguese, Spanish, Japanese, and Chinese. The language selector follows the
browser language on first visit and remembers the visitor's choice.

## GitHub Pages deployment

The `Publish marketing site` workflow uploads `site/` and deploys it to GitHub
Pages when site files change on `master`, or when manually dispatched.

In the repository's **Settings → Pages**, set **Build and deployment → Source**
to **GitHub Actions**. Merge the site and workflow to `master`; the workflow
publishes the landing page and exposes its URL in the `github-pages`
environment.

The hero video is a locally bundled, silent MP4 with a poster image. It loops
muted, provides native playback controls, and is not downloaded from a third
party.
