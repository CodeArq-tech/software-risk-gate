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

The hero plays the rendered product demo in the selected language. The files in
`assets/demo/` are symbolic links to the videos and posters in `docs/demo/`;
`actions/upload-pages-artifact` follows them, so the deployed site carries
real copies. English, Portuguese, and Spanish have their own cut; Japanese and
Chinese play the English cut. The video starts muted and loops, with native
playback controls, and no media is loaded from a third party.

To add a cut for another language, render it with
`docs/demo/video/scripts/render.sh`, link it in `assets/demo/`, and map the
site language to it in `demoVideo` in `assets/app.js`.

Preview locally with any static server that supports HTTP range requests
(needed for seeking), serving `site/` as the root.
