# Risk Gate demo videos

A 64-second walkthrough of Risk Gate running as a GitHub Actions step:
versioning `.risk-gate.yaml`, adding the workflow, opening a pull request,
watching the gate fail on a cascade into payments, reading the report, and
merging after a fix.

| Language | Video | Poster |
| --- | --- | --- |
| English | [risk-gate-demo-en.mp4](risk-gate-demo-en.mp4) | [risk-gate-demo-en.jpg](risk-gate-demo-en.jpg) |
| Português (Brasil) | [risk-gate-demo-pt-BR.mp4](risk-gate-demo-pt-BR.mp4) | [risk-gate-demo-pt-BR.jpg](risk-gate-demo-pt-BR.jpg) |
| Español | [risk-gate-demo-es.mp4](risk-gate-demo-es.mp4) | [risk-gate-demo-es.jpg](risk-gate-demo-es.jpg) |

Only the captions, intro, and closing card are translated. The GitHub
screens, code, logs, and report stay in English because they reproduce what
users see.

## What is real and what is staged

- The report comment, the Actions log, and the cascade diagram come from a
  real run of the `risk-gate` v0.1.0 release binary (`--base main --head HEAD`)
  on a fixture repository (`acme/storefront`, 40 files) whose PR changes
  `src/api/checkout.ts`, which imports `src/payment/charge.ts`. The captured
  Markdown is `video/assets/data/report-failed.md`. Validation timings are
  those of the fixture's scripts.
- The `.risk-gate.yaml` and workflow shown in the video are the fixture's
  files and match the configuration contract in `specs/configuration.md`.
- The GitHub interface is rebuilt in HTML for the video; it is not a screen
  recording. The merge box assumes branch protection that requires the Risk
  Gate check, which Risk Gate itself does not configure.
- After the fix commit the video shows only the passing check and the merge
  box. In the fixture the re-run reports `Status: PARTIAL` with
  `Merge blocked: false`, because non-code files such as `.risk-gate.yaml` and
  shell scripts are listed as unanalyzed; the updated comment is not shown.
- Music and sound effects are synthesized by `video/scripts/make_audio.py`
  with a fixed seed, so they carry no third-party licence.

## Rebuilding

The source is a [HyperFrames](https://hyperframes.heygen.com) project in
`video/`:

- `index.html` places the scenes, captions, and audio on the timeline.
- `compositions/` holds one sub-composition per scene plus the caption track.
- `assets/kit.js` is the shared seek-safe engine (cursor, camera zoom,
  typing); `assets/kit.css` rebuilds the GitHub surfaces.
- `assets/i18n.js` holds every translated string.

Preview and edit in HyperFrames Studio:

~~~bash
cd docs/demo/video
npx --yes hyperframes@0.8.155 preview
~~~

Render all languages (Node.js 22+, FFmpeg, Python 3 with NumPy; run outside
sandboxes that block Chrome):

~~~bash
docs/demo/video/scripts/render.sh          # en, pt-BR, es
docs/demo/video/scripts/render.sh pt-BR    # a single language
~~~

To add a language, add a table to `assets/i18n.js`, add it to the `lang`
options in `index.html`, and pass it to `render.sh`.
