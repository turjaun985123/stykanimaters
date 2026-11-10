# Sample content (Roadmap E-2, Wave 2 seat W2-92)

A complete, playable demo episode in the app's own format
(format_version 1), shipped as sample content — not a starter:
where `assets/templates/` files are scaffolds a new project
begins from, this is a finished miniature a user opens to see
what the app does. The file is plain project JSON exactly as
`projects::save` writes it, loads through `projects::load`,
and is pinned by `tests/w2_sample_episode.rs`.

| File | Project name | Contents |
|---|---|---|
| `the-bus-stop.json` | The Bus Stop | A 96-frame episode at 24 fps in two chapters ("Waiting" [0, 48), "The Bus Arrives" [48, 96) — real `Project::chapters` entries). Two stick figures with unique colours (Pip `0xFF9F43`, Rue `0x4FC3F7`) on the house humanoid rig: Pip waves, then steps toward Rue as the bus comes; Rue waves back. Three subtitle cues with speakers, each inside its chapter, at 15–19 CPS. A 3-key camera move (pan right + push-in 1.0 → 1.3, Smooth segment easing). One placed sprite: a 16×16 amber sun disc (RGBA PNG encoded for this sample, stored as the project's asset 0) keyed at [118, 104], scale 3. |

Sample content is data, not code: editing it edits the demo.
Keep the loader contract — every frame carries one pose per
figure, every pose one angle per node, chapters and cues
satisfy `Chapter::validate` / `SubtitleCue::validate`, sprite
asset indices stay in range — or the pin tests fail in CI,
by design.
