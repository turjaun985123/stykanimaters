# Starter templates (Roadmap E-1, Wave 2 seat W2-91)

Real project files in the app's own format (format_version 1),
offered by the Home New-project flow as starting points. Each
file is plain project JSON exactly as `projects::save` writes
it, and each loads through `projects::load` — pinned by
`tests/w2_templates.rs`.

| File | Project name | Contents |
|---|---|---|
| `episode-blank.json` | Blank Episode | Episode scaffold: 3 chapters × 16 frames at 24 fps, a held title frame (duration 24) opening each chapter, a chapter title card (text instance, opacity-keyed to its own chapter only), one standing figure, static camera. |
| `dialogue-scene.json` | Dialogue Scene | Two figures (Nova, Bram) in conversation over 144 frames: gesture beats baked as smooth interpolated frames, three subtitle cues with speakers, static wide camera. |
| `acting-scene.json` | Two-Figure Acting Scene | Mara walks to Theo (baked leg-swing cycle), arm-raise exchange, and a 4-key camera move (pan right + push-in 1.0 → 1.4) over 120 frames. |

Templates are data, not code: editing one edits what new
projects start from. Keep the loader contract — every frame
carries one pose per figure, every pose one angle per node,
subtitle cues satisfy `SubtitleCue::validate` — or the pin
tests fail in CI, by design.
