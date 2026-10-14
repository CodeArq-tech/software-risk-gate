#!/usr/bin/env bash
# Renders the Risk Gate demo in every shipped language.
#
# Requires Node.js 22+, FFmpeg, Python 3 with NumPy, and a Chrome that can
# launch (HyperFrames downloads one when needed). Run it from a normal
# terminal: sandboxes that block Chrome's Mach ports on macOS make it fail.
#
# Usage: docs/demo/video/scripts/render.sh [lang ...]   (default: en pt-BR es)
set -euo pipefail

cd "$(dirname "$0")/.."
HF="npx --yes hyperframes@0.8.155"
OUT_DIR=".."
LANGS=("$@")
if [ ${#LANGS[@]} -eq 0 ]; then
  LANGS=(en pt-BR es)
fi

restore_lang() {
  printf '/* Overwritten by scripts/render.sh for each language; empty means "use the lang variable". */\nwindow.RG_LANG = "";\n' > assets/lang.js
}
trap restore_lang EXIT

python3 scripts/make_audio.py
for track in music sfx; do
  ffmpeg -loglevel error -y -i "assets/audio/${track}.wav" -c:a aac -b:a 192k "assets/audio/${track}.m4a"
done
node scripts/report-data.mjs

if [ "${SKIP_CHECK:-0}" != "1" ]; then
  $HF check
fi

for lang in "${LANGS[@]}"; do
  printf 'window.RG_LANG = "%s";\n' "$lang" > assets/lang.js
  out="${OUT_DIR}/risk-gate-demo-${lang}.mp4"
  $HF render --quality delivery --crf 20 --fps 30 --strict \
    --variables "{\"lang\":\"${lang}\"}" --output "$out"
  test -s "$out"
  ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1 "$out"
  # Poster frame: the cascade diagram with its caption on screen.
  ffmpeg -loglevel error -y -ss 45.5 -i "$out" -frames:v 1 -q:v 3 "${OUT_DIR}/risk-gate-demo-${lang}.jpg"
done

echo "Rendered: ${LANGS[*]}"
