# Bundled fonts

Fallback faces embedded in the binary (Roadmap Wave 2,
W2-46, closing IA-RM2-6) so shaped text renders for scripts
a stock machine may have no system font for. The font
engine (src/media.rs) consults them only after the system
scan: a system face proving full coverage wins; a bundled
face beats a partially covering system face.

## Embedded faces

| File | Family | Source | Licence |
|---|---|---|---|
| NotoSansBengali-Regular.ttf (108,268 B) | Noto Sans Bengali, Regular | google/fonts `ofl/notosansbengali` variable font, instanced wght=400/wdth=100, subset to U+0980–09FF + shared punctuation | SIL OFL 1.1 — `OFL-NotoSansBengali.txt` |
| NotoSansArabic-Regular.ttf (138,004 B) | Noto Sans Arabic, Regular | google/fonts `ofl/notosansarabic` variable font, instanced wght=400/wdth=100, subset to U+0600–06FF, U+0750–077F, U+08A0–08FF, U+FB50–FDFF, U+FE70–FEFF + shared punctuation | SIL OFL 1.1 — `OFL-NotoSansArabic.txt` |
| NotoSansHebrew-Regular.ttf (27,604 B) | Noto Sans Hebrew, Regular | google/fonts `ofl/notosanshebrew` variable font, instanced wght=400/wdth=100, subset to U+0590–05FF, U+FB1D–FB4F + shared punctuation | SIL OFL 1.1 — `OFL-NotoSansHebrew.txt` |

Subsetting with fontTools (`subset` + `varLib.instancer`),
all layout features retained, no hinting. The OFL requires
modified versions not use the Reserved Font Name: no
Reserved Font Name is declared for the Sans Bengali/Arabic/
Hebrew families, and each subset keeps its original family
name and copyright notices. Full audit: the W2-46 findings
in the wave record.

## Not yet embedded

**Noto Sans SC (Han).** Licence audited clean (SIL OFL
1.1, Adobe copyright with Reserved Font Name 'Source' —
which the unmodified family name does not use); licence
text kept here as `OFL-NotoSansSC.txt`. The honest Han
subset is megabytes, not kilobytes, so embedding waits on
the size decision in IA-W2-46-1; until then Han resolves
from system fonts only.
