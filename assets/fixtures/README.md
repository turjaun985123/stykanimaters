# Stress fixtures (Roadmap PL-13, Wave 2 seat W2-74)

Project files in the app's own format (format_version 1) whose job
is to be LARGE, not lovely: they exist so long-project behaviour —
load time, timeline element growth, dopesheet density, export
weight — is measured against a real file instead of extrapolated
from the 5-frame demo. They are not starter templates
(`assets/templates/`) and not user-facing samples
(`assets/samples/`); nothing in the Home flow offers them. The
headless scene that loads one is integrator wiring (see
`findings2/w2-74.md`, IA-W2-74-1).

| File | Project name | Contents |
|---|---|---|
| `long-project.json` | The Long Platform (Stress Fixture) | 600 frames at 24 fps (25 s), stage 1280×720. **12 figures**: ten house humanoids (unique colours; four length/thickness variants), one 7-node simple figure ("Tot"), one 13-node quadruped ("Biscuit") — every frame carries one pose per figure, one angle per node (7,200 poses, dense idiom). Baked crowd motion: sway, staggered arm-raise cameos, a walk-in-place pair in chapter 3, one squash-and-stretch beat (24 poses carry `sx`/`sy`), segment bends on 180 frames (figures 4 and 9). Five chapters partitioning [0, 600). Forty subtitle cues, speakers all cast members, max 20 CPS. Eleven camera keys (pan + push-in, one Hold segment). Six graph channels (Angle/RootX/RootY/Bend). Two PNG assets (spec-encoded) placed as three sprites, one title text instance, two stored guides. 2,599,581 bytes. |

Fixtures are data, not code: editing one edits the stress test.
Keep the loader contract — every frame carries one pose per
figure, every pose one angle per node of that figure's rig,
chapters and cues satisfy `Chapter::validate` /
`SubtitleCue::validate`, sprite asset indices stay in range —
or the pin tests in `tests/w2_long_project.rs` fail in CI,
by design.
