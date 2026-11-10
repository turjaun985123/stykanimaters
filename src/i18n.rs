//! Localisation string tables for the editor UI.
//!
//! Keys are stable identifiers named after the control or message
//! they label (see `src/app.rs` for the controls and `src/editor.rs`
//! for the status strings). `t` resolves a key in the requested
//! language, falls back to English when a translation is missing,
//! and finally falls back to the key itself so an untranslated UI
//! still shows something debuggable instead of a blank.
//!
//! The language set mirrors the original app's locale packs
//! (`src/locales/packs/` in StykAnimate-App: 27 packs) plus Bengali,
//! which the port added for its user — 28 languages in total.
//! Every table carries exactly the same 99 keys.
//!
//! Translation approach: established animation terms are used where
//! a language has them (French "pelure d'oignon", Polish "skórka
//! cebuli", Chinese 洋葱皮 for onion skin); where a concept has no
//! established local term, it is transliterated or kept as a
//! loanword (Bengali টুইন, Russian Твининг for tween), matching how
//! that language's own software community writes it. File-format
//! names (PNG, GIF, MP4, WebM, SVG, .piv, .stk) stay in Latin
//! script everywhere, as they do in localised software generally.
//!
//! The Wave 9 panel keys (`help`, `inspector`, `easing`,
//! `colour_picker`, `export_centre`, `dopesheet`, `hud_loop`,
//! `transport_first`, `transport_last`, `fx_panel`, `clips_panel`,
//! `weather_panel`) are fully translated in English, Bengali,
//! Spanish, German, French, Italian, Portuguese (Brazil), Russian
//! and Chinese (Simplified) since Wave 10, and in the remaining 19
//! languages since Wave 11 — all 28 tables now carry genuine
//! translations for these keys (a few values are established
//! loanwords that legitimately match the English spelling, e.g.
//! "Easing" in several languages).
//!
//! The Wave 14 workspace keys (`ws_animate`, `ws_draw`,
//! `ws_build`, `ws_graph`, `ws_effects`, `ws_audio`, `ws_3d`,
//! `ws_export`) carry genuine translations in English, Bengali,
//! Spanish, German, French, Italian, Portuguese (Brazil), Russian
//! and Chinese (Simplified); the remaining 19 languages carry the
//! English text as a placeholder for these keys, to be translated
//! in a later pass. These keys are localisation infrastructure
//! only for now: the app bar's workspace chips still use the
//! English names as their labels this wave, like every other
//! button in the app, so the chips are not yet localised.
//!
//! The UI-mission keys (`live_mcp_control`, `live_mcp_off`,
//! `discard_changes`, `rename`, `camera_delete_key`,
//! `clear_key_no_undo`, `go_to_start`, `go_to_end`,
//! `delete_frame_strokes`, `clip_missing_file`,
//! `clip_unsupported_format`, `clip_unreadable_file`,
//! `brush_preset`, `preset_size`, `preset_opacity`,
//! `preset_scatter`, `preset_nib`, `preset_smoothing`) were
//! added by the Wave 9 completeness audit of the labels the
//! UI mission (Waves 1–8) and live MCP introduced. Their
//! English values are the exact strings at the call sites
//! (settings toggle, discard confirmations, camera key row,
//! transport jumps, audio clip-health words, brush preset
//! cyclers); every other language carries the English text
//! as a placeholder, the same convention as the Wave 14
//! workspace keys, for a later translation pass. Like the
//! workspace keys they are infrastructure for now: the call
//! sites still render the English literals until a wiring
//! pass routes them through `tr`.
//!
//! The dopesheet keys (`next_keyframe`, `previous_keyframe`,
//! `no_keyframes_yet`) were added in Wave 10 for the rebuilt
//! dopesheet panel: the first two label its keyframe
//! navigation, the third is its empty state when a figure has
//! no keyframes. Their English values are the exact strings
//! the panel shows; every other language carries the English
//! text as a placeholder, the same convention as the
//! UI-mission keys above, for a later translation pass. The
//! panel's title needs no new key: it resolves the existing
//! `dopesheet` key — genuinely translated in all 28
//! languages — through the same `tr` accessor.
//!
//! The Rune shell keys (`settings`, `frame`, `frames`,
//! `figures`, `prev_frame`, `next_frame`, `fps`, `zoom`) were
//! added in Phase E of the Rune migration, with genuine
//! translations in all 28 languages, so the Rune shell's
//! `SHELL_KEYS` list (src/rune_sec/i18n_wire.rs) is now fully
//! covered. Each value is built from that language's own
//! existing vocabulary in these tables: the frame word from
//! `add_frame` / `transport_first`, the figures plural from
//! `figures_layers`, prev/next on the `transport_first` /
//! `transport_last` pattern, zoom from `cam_zoom_plus`. Two
//! exceptions by nature: `fps` is the abbreviation "FPS" in
//! every language's animation software (Chinese uses 帧率),
//! and a few languages do not mark plurals, so `frames` matches
//! `frame` there (Greek, Indonesian, Vietnamese, Chinese).

/// Languages the UI can be displayed in: the original app's 27
/// locale packs, plus Bengali (the port's own addition).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Bn,
    Es,
    Ar,
    Cs,
    Da,
    De,
    El,
    Es419,
    Fi,
    Fr,
    He,
    Hr,
    Hu,
    Id,
    It,
    Kk,
    Nb,
    Nl,
    Pl,
    PtBr,
    Ro,
    Ru,
    Sl,
    Tr,
    Uk,
    Vi,
    ZhHans,
}

impl Lang {
    /// BCP 47 code, e.g. for a settings file or `<html lang>`.
    pub fn code(&self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Bn => "bn",
            Lang::Es => "es",
            Lang::Ar => "ar",
            Lang::Cs => "cs",
            Lang::Da => "da",
            Lang::De => "de",
            Lang::El => "el",
            Lang::Es419 => "es-419",
            Lang::Fi => "fi",
            Lang::Fr => "fr",
            Lang::He => "he",
            Lang::Hr => "hr",
            Lang::Hu => "hu",
            Lang::Id => "id",
            Lang::It => "it",
            Lang::Kk => "kk",
            Lang::Nb => "nb",
            Lang::Nl => "nl",
            Lang::Pl => "pl",
            Lang::PtBr => "pt-BR",
            Lang::Ro => "ro",
            Lang::Ru => "ru",
            Lang::Sl => "sl",
            Lang::Tr => "tr",
            Lang::Uk => "uk",
            Lang::Vi => "vi",
            Lang::ZhHans => "zh-Hans",
        }
    }

    /// The language's name in itself, for a language picker.
    pub fn label(&self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Bn => "বাংলা",
            Lang::Es => "Español",
            Lang::Ar => "العربية",
            Lang::Cs => "Čeština",
            Lang::Da => "Dansk",
            Lang::De => "Deutsch",
            Lang::El => "Ελληνικά",
            Lang::Es419 => "Español (Latinoamérica)",
            Lang::Fi => "Suomi",
            Lang::Fr => "Français",
            Lang::He => "עברית",
            Lang::Hr => "Hrvatski",
            Lang::Hu => "Magyar",
            Lang::Id => "Bahasa Indonesia",
            Lang::It => "Italiano",
            Lang::Kk => "Қазақша",
            Lang::Nb => "Norsk bokmål",
            Lang::Nl => "Nederlands",
            Lang::Pl => "Polski",
            Lang::PtBr => "Português (Brasil)",
            Lang::Ro => "Română",
            Lang::Ru => "Русский",
            Lang::Sl => "Slovenščina",
            Lang::Tr => "Türkçe",
            Lang::Uk => "Українська",
            Lang::Vi => "Tiếng Việt",
            Lang::ZhHans => "简体中文",
        }
    }

    /// All languages, in picker order.
    pub fn all() -> [Lang; 28] {
        [
            Lang::En,
            Lang::Bn,
            Lang::Es,
            Lang::Ar,
            Lang::Cs,
            Lang::Da,
            Lang::De,
            Lang::El,
            Lang::Es419,
            Lang::Fi,
            Lang::Fr,
            Lang::He,
            Lang::Hr,
            Lang::Hu,
            Lang::Id,
            Lang::It,
            Lang::Kk,
            Lang::Nb,
            Lang::Nl,
            Lang::Pl,
            Lang::PtBr,
            Lang::Ro,
            Lang::Ru,
            Lang::Sl,
            Lang::Tr,
            Lang::Uk,
            Lang::Vi,
            Lang::ZhHans,
        ]
    }
}

fn lookup<'a>(table: &'a [(&'a str, &'a str)], key: &str) -> Option<&'a str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

fn table_for(lang: Lang) -> &'static [(&'static str, &'static str)] {
    match lang {
        Lang::En => EN,
        Lang::Bn => BN,
        Lang::Es => ES,
        Lang::Ar => AR,
        Lang::Cs => CS,
        Lang::Da => DA,
        Lang::De => DE,
        Lang::El => EL,
        Lang::Es419 => ES419,
        Lang::Fi => FI,
        Lang::Fr => FR,
        Lang::He => HE,
        Lang::Hr => HR,
        Lang::Hu => HU,
        Lang::Id => ID,
        Lang::It => IT,
        Lang::Kk => KK,
        Lang::Nb => NB,
        Lang::Nl => NL,
        Lang::Pl => PL,
        Lang::PtBr => PT_BR,
        Lang::Ro => RO,
        Lang::Ru => RU,
        Lang::Sl => SL,
        Lang::Tr => TR,
        Lang::Uk => UK,
        Lang::Vi => VI,
        Lang::ZhHans => ZH_HANS,
    }
}

/// Resolve a key for `lang`: that language's table first, then
/// English, then `None` (the caller decides the last resort).
fn resolve(lang: Lang, key: &str) -> Option<&'static str> {
    lookup(table_for(lang), key).or_else(|| {
        if lang == Lang::En {
            None
        } else {
            lookup(EN, key)
        }
    })
}

/// Translate `key` into `lang`. Missing translations fall back to
/// English; a key missing everywhere falls back to the key itself.
pub fn t(lang: Lang, key: &str) -> &'static str {
    match resolve(lang, key) {
        Some(text) => text,
        // Only reachable for keys absent from every table (a bug in
        // the caller). Leaking one small string per distinct bad key
        // keeps the `&'static str` signature without a registry.
        None => Box::leak(key.to_string().into_boxed_str()),
    }
}

static EN: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Play"),
    ("pause", "Pause"),
    ("add_frame", "+ Frame"),
    ("duplicate_frame", "Duplicate"),
    ("delete_frame", "Delete"),
    ("inbetween", "Inbetween"),
    ("copy_frame", "Copy frame"),
    ("paste_frame", "Paste frame"),
    // History
    ("undo", "Undo"),
    ("redo", "Redo"),
    // Figure building
    ("add_figure", "+ Figure"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Len +"),
    ("len_minus", "Len -"),
    ("thick_plus", "Thick +"),
    ("thick_minus", "Thick -"),
    ("shape", "Shape"),
    // Canvas modes and drawing
    ("onion_skin", "Onion skin"),
    ("draw", "Draw"),
    ("clear_drawing", "Clear drawing"),
    ("camera_mode", "Camera mode"),
    ("cam_zoom_plus", "Cam zoom +"),
    ("cam_zoom_minus", "Cam zoom -"),
    ("follow_cam", "Follow cam"),
    ("tween_play", "Tween play"),
    // Particle effects
    ("fx_burst", "FX Burst"),
    ("fx_sparkle", "FX Sparkle"),
    ("fx_smoke", "FX Smoke"),
    ("clear_fx", "Clear FX"),
    // Files and export
    ("save", "Save"),
    ("load", "Load"),
    ("export_png", "Export PNG"),
    ("export_gif", "Export GIF"),
    ("export_mp4", "Export MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", "Import .piv"),
    ("import_stk", "Import .stk"),
    ("export_piv", "Export .piv"),
    ("export_stk", "Export .stk"),
    // Audio and movieclips
    ("add_audio", "Add audio"),
    ("make_clip", "Make clip"),
    ("insert_clip", "Insert clip"),
    // Themes and panels
    ("light_theme", "Light theme"),
    ("dark_theme", "Dark theme"),
    ("figures_layers", "FIGURES (layers)"),
    // Panels and help (Wave 9 UI)
    ("help", "Help"),
    ("inspector", "Inspector"),
    ("easing", "Easing"),
    ("colour_picker", "Colour picker"),
    ("export_centre", "Export centre"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Loop"),
    ("transport_first", "First frame"),
    ("transport_last", "Last frame"),
    ("fx_panel", "FX panel"),
    ("clips_panel", "Clips panel"),
    ("weather_panel", "Weather panel"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Settings"),
    ("frame", "Frame"),
    ("frames", "Frames"),
    ("figures", "Figures"),
    ("prev_frame", "Previous frame"),
    ("next_frame", "Next frame"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Pose mode."),
    (
        "status_ready",
        "Drag a joint to pose. Hands and feet use IK; drag the hips to move.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static BN: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "চালান"),
    ("pause", "বিরতি"),
    ("add_frame", "+ ফ্রেম"),
    ("duplicate_frame", "অনুলিপি"),
    ("delete_frame", "মুছুন"),
    ("inbetween", "মধ্যবর্তী ফ্রেম"),
    ("copy_frame", "ফ্রেম কপি"),
    ("paste_frame", "ফ্রেম পেস্ট"),
    // History
    ("undo", "আনডু"),
    ("redo", "রিডু"),
    // Figure building
    ("add_figure", "+ ফিগার"),
    ("add_segment", "+ অংশ"),
    ("delete_segment", "- অংশ"),
    ("len_plus", "দৈর্ঘ্য +"),
    ("len_minus", "দৈর্ঘ্য -"),
    ("thick_plus", "পুরুত্ব +"),
    ("thick_minus", "পুরুত্ব -"),
    ("shape", "আকৃতি"),
    // Canvas modes and drawing
    ("onion_skin", "ওনিয়ন স্কিন"),
    ("draw", "আঁকুন"),
    ("clear_drawing", "আঁকা মুছুন"),
    ("camera_mode", "ক্যামেরা মোড"),
    ("cam_zoom_plus", "ক্যামেরা জুম +"),
    ("cam_zoom_minus", "ক্যামেরা জুম -"),
    ("follow_cam", "ক্যামেরা অনুসরণ"),
    ("tween_play", "টুইন চালান"),
    // Particle effects
    ("fx_burst", "এফএক্স বিস্ফোরণ"),
    ("fx_sparkle", "এফএক্স ঝিলিক"),
    ("fx_smoke", "এফএক্স ধোঁয়া"),
    ("clear_fx", "এফএক্স মুছুন"),
    // Files and export
    ("save", "সংরক্ষণ"),
    ("load", "লোড"),
    ("export_png", "PNG রপ্তানি"),
    ("export_gif", "GIF রপ্তানি"),
    ("export_mp4", "MP4 রপ্তানি"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "স্প্রাইটশিট"),
    ("import_piv", ".piv আমদানি"),
    ("import_stk", ".stk আমদানি"),
    ("export_piv", ".piv রপ্তানি"),
    ("export_stk", ".stk রপ্তানি"),
    // Audio and movieclips
    ("add_audio", "অডিও যোগ করুন"),
    ("make_clip", "ক্লিপ তৈরি"),
    ("insert_clip", "ক্লিপ বসান"),
    // Themes and panels
    ("light_theme", "হালকা থিম"),
    ("dark_theme", "গাঢ় থিম"),
    ("figures_layers", "ফিগারসমূহ (স্তর)"),
    // Panels and help (Wave 9 UI)
    ("help", "সাহায্য"),
    ("inspector", "পরিদর্শক"),
    ("easing", "ইজিং"),
    ("colour_picker", "রঙ নির্বাচক"),
    ("export_centre", "রপ্তানি কেন্দ্র"),
    ("dopesheet", "ডোপশিট"),
    ("hud_loop", "লুপ"),
    ("transport_first", "প্রথম ফ্রেম"),
    ("transport_last", "শেষ ফ্রেম"),
    ("fx_panel", "এফএক্স প্যানেল"),
    ("clips_panel", "ক্লিপ প্যানেল"),
    ("weather_panel", "আবহাওয়া প্যানেল"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "অ্যানিমেট"),
    ("ws_draw", "আঁকুন"),
    ("ws_build", "তৈরি"),
    ("ws_graph", "গ্রাফ"),
    ("ws_effects", "ইফেক্টস"),
    ("ws_audio", "অডিও"),
    ("ws_3d", "3D"),
    ("ws_export", "এক্সপোর্ট"),
    ("ws_home", "হোম"),
    // Rune shell labels (Phase E)
    ("settings", "সেটিংস"),
    ("frame", "ফ্রেম"),
    ("frames", "ফ্রেমসমূহ"),
    ("figures", "ফিগারসমূহ"),
    ("prev_frame", "আগের ফ্রেম"),
    ("next_frame", "পরের ফ্রেম"),
    ("fps", "FPS"),
    ("zoom", "জুম"),
    // Status line
    ("pose_mode", "পোজ মোড।"),
    (
        "status_ready",
        "পোজ দেওয়ার জন্য একটি জয়েন্ট টেনে ধরুন। হাত ও পা IK দিয়ে চলে; ফিগার সরাতে কোমর টানুন।",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static ES: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Reproducir"),
    ("pause", "Pausar"),
    ("add_frame", "+ Fotograma"),
    ("duplicate_frame", "Duplicar"),
    ("delete_frame", "Eliminar"),
    ("inbetween", "Intermedio"),
    ("copy_frame", "Copiar fotograma"),
    ("paste_frame", "Pegar fotograma"),
    // History
    ("undo", "Deshacer"),
    ("redo", "Rehacer"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segmento"),
    ("delete_segment", "- Segmento"),
    ("len_plus", "Long. +"),
    ("len_minus", "Long. -"),
    ("thick_plus", "Grosor +"),
    ("thick_minus", "Grosor -"),
    ("shape", "Forma"),
    // Canvas modes and drawing
    ("onion_skin", "Piel de cebolla"),
    ("draw", "Dibujar"),
    ("clear_drawing", "Borrar dibujo"),
    ("camera_mode", "Modo cámara"),
    ("cam_zoom_plus", "Zoom cámara +"),
    ("cam_zoom_minus", "Zoom cámara -"),
    ("follow_cam", "Seguir cámara"),
    ("tween_play", "Interpolación"),
    // Particle effects
    ("fx_burst", "FX Explosión"),
    ("fx_sparkle", "FX Destellos"),
    ("fx_smoke", "FX Humo"),
    ("clear_fx", "Borrar FX"),
    // Files and export
    ("save", "Guardar"),
    ("load", "Cargar"),
    ("export_png", "Exportar PNG"),
    ("export_gif", "Exportar GIF"),
    ("export_mp4", "Exportar MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Hoja de sprites"),
    ("import_piv", "Importar .piv"),
    ("import_stk", "Importar .stk"),
    ("export_piv", "Exportar .piv"),
    ("export_stk", "Exportar .stk"),
    // Audio and movieclips
    ("add_audio", "Añadir audio"),
    ("make_clip", "Crear clip"),
    ("insert_clip", "Insertar clip"),
    // Themes and panels
    ("light_theme", "Tema claro"),
    ("dark_theme", "Tema oscuro"),
    ("figures_layers", "FIGURAS (capas)"),
    // Panels and help (Wave 9 UI)
    ("help", "Ayuda"),
    ("inspector", "Inspector"),
    ("easing", "Suavizado"),
    ("colour_picker", "Selector de color"),
    ("export_centre", "Centro de exportación"),
    ("dopesheet", "Hoja de tiempos"),
    ("hud_loop", "Bucle"),
    ("transport_first", "Primer fotograma"),
    ("transport_last", "Último fotograma"),
    ("fx_panel", "Panel de FX"),
    ("clips_panel", "Panel de clips"),
    ("weather_panel", "Panel de clima"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animar"),
    ("ws_draw", "Dibujar"),
    ("ws_build", "Construir"),
    ("ws_graph", "Gráfico"),
    ("ws_effects", "Efectos"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Exportar"),
    ("ws_home", "Inicio"),
    // Rune shell labels (Phase E)
    ("settings", "Ajustes"),
    ("frame", "Fotograma"),
    ("frames", "Fotogramas"),
    ("figures", "Figuras"),
    ("prev_frame", "Fotograma anterior"),
    ("next_frame", "Fotograma siguiente"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Modo pose."),
    (
        "status_ready",
        "Arrastra una articulación para posar. Las manos y los pies usan IK; arrastra las caderas para mover.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static AR: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "تشغيل"),
    ("pause", "إيقاف مؤقت"),
    ("add_frame", "+ إطار"),
    ("duplicate_frame", "تكرار"),
    ("delete_frame", "حذف"),
    ("inbetween", "إطار وسيط"),
    ("copy_frame", "نسخ الإطار"),
    ("paste_frame", "لصق الإطار"),
    // History
    ("undo", "تراجع"),
    ("redo", "إعادة"),
    // Figure building
    ("add_figure", "+ شكل"),
    ("add_segment", "+ مقطع"),
    ("delete_segment", "- مقطع"),
    ("len_plus", "الطول +"),
    ("len_minus", "الطول -"),
    ("thick_plus", "السُّمك +"),
    ("thick_minus", "السُّمك -"),
    ("shape", "الشكل"),
    // Canvas modes and drawing
    ("onion_skin", "قشرة البصل"),
    ("draw", "رسم"),
    ("clear_drawing", "مسح الرسم"),
    ("camera_mode", "وضع الكاميرا"),
    ("cam_zoom_plus", "تكبير الكاميرا +"),
    ("cam_zoom_minus", "تكبير الكاميرا -"),
    ("follow_cam", "تتبع الكاميرا"),
    ("tween_play", "تشغيل التوين"),
    // Particle effects
    ("fx_burst", "مؤثر انفجار"),
    ("fx_sparkle", "مؤثر بريق"),
    ("fx_smoke", "مؤثر دخان"),
    ("clear_fx", "مسح المؤثرات"),
    // Files and export
    ("save", "حفظ"),
    ("load", "فتح"),
    ("export_png", "تصدير PNG"),
    ("export_gif", "تصدير GIF"),
    ("export_mp4", "تصدير MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "ورقة السبرايت"),
    ("import_piv", "استيراد .piv"),
    ("import_stk", "استيراد .stk"),
    ("export_piv", "تصدير .piv"),
    ("export_stk", "تصدير .stk"),
    // Audio and movieclips
    ("add_audio", "إضافة صوت"),
    ("make_clip", "إنشاء مقطع"),
    ("insert_clip", "إدراج مقطع"),
    // Themes and panels
    ("light_theme", "المظهر الفاتح"),
    ("dark_theme", "المظهر الداكن"),
    ("figures_layers", "الأشكال (الطبقات)"),
    // Panels and help (Wave 9 UI)
    ("help", "مساعدة"),
    ("inspector", "المفتش"),
    ("easing", "التخفيف"),
    ("colour_picker", "منتقي الألوان"),
    ("export_centre", "مركز التصدير"),
    ("dopesheet", "ورقة التعريض"),
    ("hud_loop", "تكرار"),
    ("transport_first", "الإطار الأول"),
    ("transport_last", "الإطار الأخير"),
    ("fx_panel", "لوحة المؤثرات"),
    ("clips_panel", "لوحة المقاطع"),
    ("weather_panel", "لوحة الطقس"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "الإعدادات"),
    ("frame", "إطار"),
    ("frames", "إطارات"),
    ("figures", "الأشكال"),
    ("prev_frame", "الإطار السابق"),
    ("next_frame", "الإطار التالي"),
    ("fps", "FPS"),
    ("zoom", "تكبير"),
    // Status line
    ("pose_mode", "وضع الوضعية."),
    (
        "status_ready",
        "اسحب مفصلًا لتحديد الوضعية. تستخدم اليدان والقدمان IK؛ اسحب الوركين للتحريك.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static CS: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Přehrát"),
    ("pause", "Pozastavit"),
    ("add_frame", "+ Snímek"),
    ("duplicate_frame", "Duplikovat"),
    ("delete_frame", "Smazat"),
    ("inbetween", "Mezisnímek"),
    ("copy_frame", "Kopírovat snímek"),
    ("paste_frame", "Vložit snímek"),
    // History
    ("undo", "Zpět"),
    ("redo", "Znovu"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Délka +"),
    ("len_minus", "Délka -"),
    ("thick_plus", "Tloušťka +"),
    ("thick_minus", "Tloušťka -"),
    ("shape", "Tvar"),
    // Canvas modes and drawing
    ("onion_skin", "Cibulová slupka"),
    ("draw", "Kreslit"),
    ("clear_drawing", "Vymazat kresbu"),
    ("camera_mode", "Režim kamery"),
    ("cam_zoom_plus", "Zoom kamery +"),
    ("cam_zoom_minus", "Zoom kamery -"),
    ("follow_cam", "Sledovat kameru"),
    ("tween_play", "Interpolace"),
    // Particle effects
    ("fx_burst", "FX Výbuch"),
    ("fx_sparkle", "FX Jiskry"),
    ("fx_smoke", "FX Kouř"),
    ("clear_fx", "Vymazat FX"),
    // Files and export
    ("save", "Uložit"),
    ("load", "Načíst"),
    ("export_png", "Export PNG"),
    ("export_gif", "Export GIF"),
    ("export_mp4", "Export MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", "Import .piv"),
    ("import_stk", "Import .stk"),
    ("export_piv", "Export .piv"),
    ("export_stk", "Export .stk"),
    // Audio and movieclips
    ("add_audio", "Přidat zvuk"),
    ("make_clip", "Vytvořit klip"),
    ("insert_clip", "Vložit klip"),
    // Themes and panels
    ("light_theme", "Světlý motiv"),
    ("dark_theme", "Tmavý motiv"),
    ("figures_layers", "FIGURY (vrstvy)"),
    // Panels and help (Wave 9 UI)
    ("help", "Nápověda"),
    ("inspector", "Inspektor"),
    ("easing", "Easing"),
    ("colour_picker", "Výběr barvy"),
    ("export_centre", "Centrum exportu"),
    ("dopesheet", "Expoziční listina"),
    ("hud_loop", "Smyčka"),
    ("transport_first", "První snímek"),
    ("transport_last", "Poslední snímek"),
    ("fx_panel", "Panel FX"),
    ("clips_panel", "Panel klipů"),
    ("weather_panel", "Panel počasí"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Nastavení"),
    ("frame", "Snímek"),
    ("frames", "Snímky"),
    ("figures", "Figury"),
    ("prev_frame", "Předchozí snímek"),
    ("next_frame", "Další snímek"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Režim pózy."),
    (
        "status_ready",
        "Přetažením kloubu nastavte pózu. Ruce a nohy používají IK; postavu přesunete tažením za boky.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static DA: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Afspil"),
    ("pause", "Pause"),
    ("add_frame", "+ Billede"),
    ("duplicate_frame", "Dupliker"),
    ("delete_frame", "Slet"),
    ("inbetween", "Mellembillede"),
    ("copy_frame", "Kopiér billede"),
    ("paste_frame", "Indsæt billede"),
    // History
    ("undo", "Fortryd"),
    ("redo", "Omgør"),
    // Figure building
    ("add_figure", "+ Figur"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Længde +"),
    ("len_minus", "Længde -"),
    ("thick_plus", "Tykkelse +"),
    ("thick_minus", "Tykkelse -"),
    ("shape", "Form"),
    // Canvas modes and drawing
    ("onion_skin", "Løgskal"),
    ("draw", "Tegn"),
    ("clear_drawing", "Ryd tegning"),
    ("camera_mode", "Kameratilstand"),
    ("cam_zoom_plus", "Kamerazoom +"),
    ("cam_zoom_minus", "Kamerazoom -"),
    ("follow_cam", "Følg kamera"),
    ("tween_play", "Interpolation"),
    // Particle effects
    ("fx_burst", "FX Udbrud"),
    ("fx_sparkle", "FX Gnister"),
    ("fx_smoke", "FX Røg"),
    ("clear_fx", "Ryd FX"),
    // Files and export
    ("save", "Gem"),
    ("load", "Indlæs"),
    ("export_png", "Eksportér PNG"),
    ("export_gif", "Eksportér GIF"),
    ("export_mp4", "Eksportér MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", "Importér .piv"),
    ("import_stk", "Importér .stk"),
    ("export_piv", "Eksportér .piv"),
    ("export_stk", "Eksportér .stk"),
    // Audio and movieclips
    ("add_audio", "Tilføj lyd"),
    ("make_clip", "Opret klip"),
    ("insert_clip", "Indsæt klip"),
    // Themes and panels
    ("light_theme", "Lyst tema"),
    ("dark_theme", "Mørkt tema"),
    ("figures_layers", "FIGURER (lag)"),
    // Panels and help (Wave 9 UI)
    ("help", "Hjælp"),
    ("inspector", "Inspektør"),
    ("easing", "Easing"),
    ("colour_picker", "Farvevælger"),
    ("export_centre", "Eksportcenter"),
    ("dopesheet", "Eksponeringsark"),
    ("hud_loop", "Løkke"),
    ("transport_first", "Første billede"),
    ("transport_last", "Sidste billede"),
    ("fx_panel", "FX-panel"),
    ("clips_panel", "Klippanel"),
    ("weather_panel", "Vejrpanel"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Indstillinger"),
    ("frame", "Billede"),
    ("frames", "Billeder"),
    ("figures", "Figurer"),
    ("prev_frame", "Forrige billede"),
    ("next_frame", "Næste billede"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Poseringstilstand."),
    (
        "status_ready",
        "Træk i et led for at posere. Hænder og fødder bruger IK; træk i hofterne for at flytte.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static DE: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Abspielen"),
    ("pause", "Pause"),
    ("add_frame", "+ Frame"),
    ("duplicate_frame", "Duplizieren"),
    ("delete_frame", "Löschen"),
    ("inbetween", "Zwischenbild"),
    ("copy_frame", "Frame kopieren"),
    ("paste_frame", "Frame einfügen"),
    // History
    ("undo", "Rückgängig"),
    ("redo", "Wiederherstellen"),
    // Figure building
    ("add_figure", "+ Figur"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Länge +"),
    ("len_minus", "Länge -"),
    ("thick_plus", "Dicke +"),
    ("thick_minus", "Dicke -"),
    ("shape", "Form"),
    // Canvas modes and drawing
    ("onion_skin", "Zwiebelhaut"),
    ("draw", "Zeichnen"),
    ("clear_drawing", "Zeichnung löschen"),
    ("camera_mode", "Kameramodus"),
    ("cam_zoom_plus", "Kamera-Zoom +"),
    ("cam_zoom_minus", "Kamera-Zoom -"),
    ("follow_cam", "Kamera folgen"),
    ("tween_play", "Tween abspielen"),
    // Particle effects
    ("fx_burst", "FX Explosion"),
    ("fx_sparkle", "FX Funken"),
    ("fx_smoke", "FX Rauch"),
    ("clear_fx", "FX löschen"),
    // Files and export
    ("save", "Speichern"),
    ("load", "Laden"),
    ("export_png", "PNG exportieren"),
    ("export_gif", "GIF exportieren"),
    ("export_mp4", "MP4 exportieren"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", ".piv importieren"),
    ("import_stk", ".stk importieren"),
    ("export_piv", ".piv exportieren"),
    ("export_stk", ".stk exportieren"),
    // Audio and movieclips
    ("add_audio", "Audio hinzufügen"),
    ("make_clip", "Clip erstellen"),
    ("insert_clip", "Clip einfügen"),
    // Themes and panels
    ("light_theme", "Helles Design"),
    ("dark_theme", "Dunkles Design"),
    ("figures_layers", "FIGUREN (Ebenen)"),
    // Panels and help (Wave 9 UI)
    ("help", "Hilfe"),
    ("inspector", "Inspektor"),
    ("easing", "Easing"),
    ("colour_picker", "Farbauswahl"),
    ("export_centre", "Exportzentrum"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Schleife"),
    ("transport_first", "Erster Frame"),
    ("transport_last", "Letzter Frame"),
    ("fx_panel", "FX-Panel"),
    ("clips_panel", "Clip-Panel"),
    ("weather_panel", "Wetter-Panel"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animieren"),
    ("ws_draw", "Zeichnen"),
    ("ws_build", "Bauen"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effekte"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Exportieren"),
    ("ws_home", "Startseite"),
    // Rune shell labels (Phase E)
    ("settings", "Einstellungen"),
    ("frame", "Frame"),
    ("frames", "Frames"),
    ("figures", "Figuren"),
    ("prev_frame", "Vorheriger Frame"),
    ("next_frame", "Nächster Frame"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Posenmodus."),
    (
        "status_ready",
        "Ziehen Sie ein Gelenk, um die Pose zu setzen. Hände und Füße nutzen IK; ziehen Sie die Hüfte zum Bewegen.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static EL: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Αναπαραγωγή"),
    ("pause", "Παύση"),
    ("add_frame", "+ Καρέ"),
    ("duplicate_frame", "Διπλασιασμός"),
    ("delete_frame", "Διαγραφή"),
    ("inbetween", "Ενδιάμεσο καρέ"),
    ("copy_frame", "Αντιγραφή καρέ"),
    ("paste_frame", "Επικόλληση καρέ"),
    // History
    ("undo", "Αναίρεση"),
    ("redo", "Επανάληψη"),
    // Figure building
    ("add_figure", "+ Φιγούρα"),
    ("add_segment", "+ Τμήμα"),
    ("delete_segment", "- Τμήμα"),
    ("len_plus", "Μήκος +"),
    ("len_minus", "Μήκος -"),
    ("thick_plus", "Πάχος +"),
    ("thick_minus", "Πάχος -"),
    ("shape", "Σχήμα"),
    // Canvas modes and drawing
    ("onion_skin", "Φλούδα κρεμμυδιού"),
    ("draw", "Σχεδίαση"),
    ("clear_drawing", "Εκκαθάριση σχεδίου"),
    ("camera_mode", "Λειτουργία κάμερας"),
    ("cam_zoom_plus", "Ζουμ κάμερας +"),
    ("cam_zoom_minus", "Ζουμ κάμερας -"),
    ("follow_cam", "Ακολούθηση κάμερας"),
    ("tween_play", "Αναπαραγωγή τουίν"),
    // Particle effects
    ("fx_burst", "FX Έκρηξη"),
    ("fx_sparkle", "FX Σπινθήρες"),
    ("fx_smoke", "FX Καπνός"),
    ("clear_fx", "Εκκαθάριση FX"),
    // Files and export
    ("save", "Αποθήκευση"),
    ("load", "Φόρτωση"),
    ("export_png", "Εξαγωγή PNG"),
    ("export_gif", "Εξαγωγή GIF"),
    ("export_mp4", "Εξαγωγή MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Φύλλο σπράιτ"),
    ("import_piv", "Εισαγωγή .piv"),
    ("import_stk", "Εισαγωγή .stk"),
    ("export_piv", "Εξαγωγή .piv"),
    ("export_stk", "Εξαγωγή .stk"),
    // Audio and movieclips
    ("add_audio", "Προσθήκη ήχου"),
    ("make_clip", "Δημιουργία κλιπ"),
    ("insert_clip", "Εισαγωγή κλιπ"),
    // Themes and panels
    ("light_theme", "Ανοιχτό θέμα"),
    ("dark_theme", "Σκούρο θέμα"),
    ("figures_layers", "ΦΙΓΟΥΡΕΣ (επίπεδα)"),
    // Panels and help (Wave 9 UI)
    ("help", "Βοήθεια"),
    ("inspector", "Επιθεωρητής"),
    ("easing", "Εξομάλυνση"),
    ("colour_picker", "Επιλογή χρώματος"),
    ("export_centre", "Κέντρο εξαγωγής"),
    ("dopesheet", "Φύλλο έκθεσης"),
    ("hud_loop", "Βρόχος"),
    ("transport_first", "Πρώτο καρέ"),
    ("transport_last", "Τελευταίο καρέ"),
    ("fx_panel", "Πίνακας FX"),
    ("clips_panel", "Πίνακας κλιπ"),
    ("weather_panel", "Πίνακας καιρού"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Ρυθμίσεις"),
    ("frame", "Καρέ"),
    ("frames", "Καρέ"),
    ("figures", "Φιγούρες"),
    ("prev_frame", "Προηγούμενο καρέ"),
    ("next_frame", "Επόμενο καρέ"),
    ("fps", "FPS"),
    ("zoom", "Ζουμ"),
    // Status line
    ("pose_mode", "Λειτουργία πόζας."),
    (
        "status_ready",
        "Σύρετε μια άρθρωση για πόζα. Τα χέρια και τα πόδια χρησιμοποιούν IK· σύρετε τους γοφούς για μετακίνηση.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static ES419: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Reproducir"),
    ("pause", "Pausar"),
    ("add_frame", "+ Fotograma"),
    ("duplicate_frame", "Duplicar"),
    ("delete_frame", "Eliminar"),
    ("inbetween", "Intermedio"),
    ("copy_frame", "Copiar fotograma"),
    ("paste_frame", "Pegar fotograma"),
    // History
    ("undo", "Deshacer"),
    ("redo", "Rehacer"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segmento"),
    ("delete_segment", "- Segmento"),
    ("len_plus", "Long. +"),
    ("len_minus", "Long. -"),
    ("thick_plus", "Grosor +"),
    ("thick_minus", "Grosor -"),
    ("shape", "Forma"),
    // Canvas modes and drawing
    ("onion_skin", "Piel de cebolla"),
    ("draw", "Dibujar"),
    ("clear_drawing", "Borrar dibujo"),
    ("camera_mode", "Modo de cámara"),
    ("cam_zoom_plus", "Zoom de cámara +"),
    ("cam_zoom_minus", "Zoom de cámara -"),
    ("follow_cam", "Seguir cámara"),
    ("tween_play", "Interpolación"),
    // Particle effects
    ("fx_burst", "FX Explosión"),
    ("fx_sparkle", "FX Destellos"),
    ("fx_smoke", "FX Humo"),
    ("clear_fx", "Borrar FX"),
    // Files and export
    ("save", "Guardar"),
    ("load", "Cargar"),
    ("export_png", "Exportar PNG"),
    ("export_gif", "Exportar GIF"),
    ("export_mp4", "Exportar MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Hoja de sprites"),
    ("import_piv", "Importar .piv"),
    ("import_stk", "Importar .stk"),
    ("export_piv", "Exportar .piv"),
    ("export_stk", "Exportar .stk"),
    // Audio and movieclips
    ("add_audio", "Agregar audio"),
    ("make_clip", "Crear clip"),
    ("insert_clip", "Insertar clip"),
    // Themes and panels
    ("light_theme", "Tema claro"),
    ("dark_theme", "Tema oscuro"),
    ("figures_layers", "FIGURAS (capas)"),
    // Panels and help (Wave 9 UI)
    ("help", "Ayuda"),
    ("inspector", "Inspector"),
    ("easing", "Suavizado"),
    ("colour_picker", "Selector de color"),
    ("export_centre", "Centro de exportación"),
    ("dopesheet", "Hoja de tiempos"),
    ("hud_loop", "Bucle"),
    ("transport_first", "Primer fotograma"),
    ("transport_last", "Último fotograma"),
    ("fx_panel", "Panel de FX"),
    ("clips_panel", "Panel de clips"),
    ("weather_panel", "Panel de clima"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Ajustes"),
    ("frame", "Fotograma"),
    ("frames", "Fotogramas"),
    ("figures", "Figuras"),
    ("prev_frame", "Fotograma anterior"),
    ("next_frame", "Fotograma siguiente"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Modo de pose."),
    (
        "status_ready",
        "Arrastra una articulación para posar. Las manos y los pies usan IK; arrastra las caderas para moverlo.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static FI: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Toista"),
    ("pause", "Tauko"),
    ("add_frame", "+ Ruutu"),
    ("duplicate_frame", "Monista"),
    ("delete_frame", "Poista"),
    ("inbetween", "Väliruutu"),
    ("copy_frame", "Kopioi ruutu"),
    ("paste_frame", "Liitä ruutu"),
    // History
    ("undo", "Kumoa"),
    ("redo", "Tee uudelleen"),
    // Figure building
    ("add_figure", "+ Hahmo"),
    ("add_segment", "+ Segmentti"),
    ("delete_segment", "- Segmentti"),
    ("len_plus", "Pituus +"),
    ("len_minus", "Pituus -"),
    ("thick_plus", "Paksuus +"),
    ("thick_minus", "Paksuus -"),
    ("shape", "Muoto"),
    // Canvas modes and drawing
    ("onion_skin", "Sipulinkuori"),
    ("draw", "Piirrä"),
    ("clear_drawing", "Tyhjennä piirros"),
    ("camera_mode", "Kameratila"),
    ("cam_zoom_plus", "Kameran zoomaus +"),
    ("cam_zoom_minus", "Kameran zoomaus -"),
    ("follow_cam", "Seuraa kameraa"),
    ("tween_play", "Interpolointi"),
    // Particle effects
    ("fx_burst", "FX Räjähdys"),
    ("fx_sparkle", "FX Kipinät"),
    ("fx_smoke", "FX Savu"),
    ("clear_fx", "Tyhjennä FX"),
    // Files and export
    ("save", "Tallenna"),
    ("load", "Lataa"),
    ("export_png", "Vie PNG"),
    ("export_gif", "Vie GIF"),
    ("export_mp4", "Vie MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", "Tuo .piv"),
    ("import_stk", "Tuo .stk"),
    ("export_piv", "Vie .piv"),
    ("export_stk", "Vie .stk"),
    // Audio and movieclips
    ("add_audio", "Lisää ääni"),
    ("make_clip", "Luo leike"),
    ("insert_clip", "Lisää leike"),
    // Themes and panels
    ("light_theme", "Vaalea teema"),
    ("dark_theme", "Tumma teema"),
    ("figures_layers", "HAHMOT (tasot)"),
    // Panels and help (Wave 9 UI)
    ("help", "Ohje"),
    ("inspector", "Tarkastaja"),
    ("easing", "Easing"),
    ("colour_picker", "Värinvalitsin"),
    ("export_centre", "Vientikeskus"),
    ("dopesheet", "Valotusarkki"),
    ("hud_loop", "Silmukka"),
    ("transport_first", "Ensimmäinen ruutu"),
    ("transport_last", "Viimeinen ruutu"),
    ("fx_panel", "FX-paneeli"),
    ("clips_panel", "Leikepaneeli"),
    ("weather_panel", "Sääpaneeli"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Asetukset"),
    ("frame", "Ruutu"),
    ("frames", "Ruudut"),
    ("figures", "Hahmot"),
    ("prev_frame", "Edellinen ruutu"),
    ("next_frame", "Seuraava ruutu"),
    ("fps", "FPS"),
    ("zoom", "Zoomaus"),
    // Status line
    ("pose_mode", "Asentotila."),
    (
        "status_ready",
        "Aseta asento vetämällä niveltä. Kädet ja jalat käyttävät IK:ta; siirrä vetämällä lantiosta.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static FR: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Lire"),
    ("pause", "Pause"),
    ("add_frame", "+ Image"),
    ("duplicate_frame", "Dupliquer"),
    ("delete_frame", "Supprimer"),
    ("inbetween", "Intermédiaire"),
    ("copy_frame", "Copier l'image"),
    ("paste_frame", "Coller l'image"),
    // History
    ("undo", "Annuler"),
    ("redo", "Rétablir"),
    // Figure building
    ("add_figure", "+ Figure"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Long. +"),
    ("len_minus", "Long. -"),
    ("thick_plus", "Épaisseur +"),
    ("thick_minus", "Épaisseur -"),
    ("shape", "Forme"),
    // Canvas modes and drawing
    ("onion_skin", "Pelure d'oignon"),
    ("draw", "Dessiner"),
    ("clear_drawing", "Effacer le dessin"),
    ("camera_mode", "Mode caméra"),
    ("cam_zoom_plus", "Zoom caméra +"),
    ("cam_zoom_minus", "Zoom caméra -"),
    ("follow_cam", "Suivre la caméra"),
    ("tween_play", "Interpolation"),
    // Particle effects
    ("fx_burst", "FX Explosion"),
    ("fx_sparkle", "FX Étincelles"),
    ("fx_smoke", "FX Fumée"),
    ("clear_fx", "Effacer les FX"),
    // Files and export
    ("save", "Enregistrer"),
    ("load", "Charger"),
    ("export_png", "Exporter PNG"),
    ("export_gif", "Exporter GIF"),
    ("export_mp4", "Exporter MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Feuille de sprites"),
    ("import_piv", "Importer .piv"),
    ("import_stk", "Importer .stk"),
    ("export_piv", "Exporter .piv"),
    ("export_stk", "Exporter .stk"),
    // Audio and movieclips
    ("add_audio", "Ajouter l'audio"),
    ("make_clip", "Créer un clip"),
    ("insert_clip", "Insérer un clip"),
    // Themes and panels
    ("light_theme", "Thème clair"),
    ("dark_theme", "Thème sombre"),
    ("figures_layers", "FIGURES (calques)"),
    // Panels and help (Wave 9 UI)
    ("help", "Aide"),
    ("inspector", "Inspecteur"),
    ("easing", "Assouplissement"),
    ("colour_picker", "Sélecteur de couleur"),
    ("export_centre", "Centre d'exportation"),
    ("dopesheet", "Feuille d'exposition"),
    ("hud_loop", "Boucle"),
    ("transport_first", "Première image"),
    ("transport_last", "Dernière image"),
    ("fx_panel", "Panneau FX"),
    ("clips_panel", "Panneau des clips"),
    ("weather_panel", "Panneau météo"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animer"),
    ("ws_draw", "Dessiner"),
    ("ws_build", "Construire"),
    ("ws_graph", "Graphe"),
    ("ws_effects", "Effets"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Exporter"),
    ("ws_home", "Accueil"),
    // Rune shell labels (Phase E)
    ("settings", "Paramètres"),
    ("frame", "Image"),
    ("frames", "Images"),
    ("figures", "Figures"),
    ("prev_frame", "Image précédente"),
    ("next_frame", "Image suivante"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Mode pose."),
    (
        "status_ready",
        "Faites glisser une articulation pour poser. Les mains et les pieds utilisent l'IK ; faites glisser les hanches pour déplacer.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static HE: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "הפעלה"),
    ("pause", "השהיה"),
    ("add_frame", "+ פריים"),
    ("duplicate_frame", "שכפול"),
    ("delete_frame", "מחיקה"),
    ("inbetween", "פריים ביניים"),
    ("copy_frame", "העתקת פריים"),
    ("paste_frame", "הדבקת פריים"),
    // History
    ("undo", "בטל"),
    ("redo", "בצע שוב"),
    // Figure building
    ("add_figure", "+ דמות"),
    ("add_segment", "+ מקטע"),
    ("delete_segment", "- מקטע"),
    ("len_plus", "אורך +"),
    ("len_minus", "אורך -"),
    ("thick_plus", "עובי +"),
    ("thick_minus", "עובי -"),
    ("shape", "צורה"),
    // Canvas modes and drawing
    ("onion_skin", "קליפת בצל"),
    ("draw", "ציור"),
    ("clear_drawing", "ניקוי הציור"),
    ("camera_mode", "מצב מצלמה"),
    ("cam_zoom_plus", "זום מצלמה +"),
    ("cam_zoom_minus", "זום מצלמה -"),
    ("follow_cam", "מעקב מצלמה"),
    ("tween_play", "ניגון טווין"),
    // Particle effects
    ("fx_burst", "אפקט התפוצצות"),
    ("fx_sparkle", "אפקט ניצוצות"),
    ("fx_smoke", "אפקט עשן"),
    ("clear_fx", "ניקוי אפקטים"),
    // Files and export
    ("save", "שמירה"),
    ("load", "טעינה"),
    ("export_png", "ייצוא PNG"),
    ("export_gif", "ייצוא GIF"),
    ("export_mp4", "ייצוא MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "גיליון ספרייטים"),
    ("import_piv", "ייבוא .piv"),
    ("import_stk", "ייבוא .stk"),
    ("export_piv", "ייצוא .piv"),
    ("export_stk", "ייצוא .stk"),
    // Audio and movieclips
    ("add_audio", "הוספת שמע"),
    ("make_clip", "יצירת קליפ"),
    ("insert_clip", "הוספת קליפ"),
    // Themes and panels
    ("light_theme", "ערכת נושא בהירה"),
    ("dark_theme", "ערכת נושא כהה"),
    ("figures_layers", "דמויות (שכבות)"),
    // Panels and help (Wave 9 UI)
    ("help", "עזרה"),
    ("inspector", "מפקח"),
    ("easing", "הקלה"),
    ("colour_picker", "בורר צבעים"),
    ("export_centre", "מרכז הייצוא"),
    ("dopesheet", "גיליון חשיפה"),
    ("hud_loop", "לולאה"),
    ("transport_first", "הפריים הראשון"),
    ("transport_last", "הפריים האחרון"),
    ("fx_panel", "פאנל אפקטים"),
    ("clips_panel", "פאנל קליפים"),
    ("weather_panel", "פאנל מזג האוויר"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "הגדרות"),
    ("frame", "פריים"),
    ("frames", "פריימים"),
    ("figures", "דמויות"),
    ("prev_frame", "הפריים הקודם"),
    ("next_frame", "הפריים הבא"),
    ("fps", "FPS"),
    ("zoom", "זום"),
    // Status line
    ("pose_mode", "מצב פוזה."),
    (
        "status_ready",
        "גררו מפרק כדי לקבוע פוזה. הידיים והרגליים משתמשות ב-IK; גררו את הירכיים כדי להזיז.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static HR: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Reproduciraj"),
    ("pause", "Pauza"),
    ("add_frame", "+ Sličica"),
    ("duplicate_frame", "Dupliciraj"),
    ("delete_frame", "Izbriši"),
    ("inbetween", "Međusličica"),
    ("copy_frame", "Kopiraj sličicu"),
    ("paste_frame", "Zalijepi sličicu"),
    // History
    ("undo", "Poništi"),
    ("redo", "Ponovi"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Duljina +"),
    ("len_minus", "Duljina -"),
    ("thick_plus", "Debljina +"),
    ("thick_minus", "Debljina -"),
    ("shape", "Oblik"),
    // Canvas modes and drawing
    ("onion_skin", "Ljuska luka"),
    ("draw", "Crtanje"),
    ("clear_drawing", "Obriši crtež"),
    ("camera_mode", "Način kamere"),
    ("cam_zoom_plus", "Zumiranje kamere +"),
    ("cam_zoom_minus", "Zumiranje kamere -"),
    ("follow_cam", "Prati kameru"),
    ("tween_play", "Interpolacija"),
    // Particle effects
    ("fx_burst", "FX Eksplozija"),
    ("fx_sparkle", "FX Iskre"),
    ("fx_smoke", "FX Dim"),
    ("clear_fx", "Obriši FX"),
    // Files and export
    ("save", "Spremi"),
    ("load", "Učitaj"),
    ("export_png", "Izvoz PNG"),
    ("export_gif", "Izvoz GIF"),
    ("export_mp4", "Izvoz MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", "Uvoz .piv"),
    ("import_stk", "Uvoz .stk"),
    ("export_piv", "Izvoz .piv"),
    ("export_stk", "Izvoz .stk"),
    // Audio and movieclips
    ("add_audio", "Dodaj zvuk"),
    ("make_clip", "Stvori isječak"),
    ("insert_clip", "Umetni isječak"),
    // Themes and panels
    ("light_theme", "Svijetla tema"),
    ("dark_theme", "Tamna tema"),
    ("figures_layers", "FIGURE (slojevi)"),
    // Panels and help (Wave 9 UI)
    ("help", "Pomoć"),
    ("inspector", "Inspektor"),
    ("easing", "Easing"),
    ("colour_picker", "Odabir boje"),
    ("export_centre", "Središte izvoza"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Ponavljanje"),
    ("transport_first", "Prva sličica"),
    ("transport_last", "Zadnja sličica"),
    ("fx_panel", "FX ploča"),
    ("clips_panel", "Ploča isječaka"),
    ("weather_panel", "Vremenska ploča"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Postavke"),
    ("frame", "Sličica"),
    ("frames", "Sličice"),
    ("figures", "Figure"),
    ("prev_frame", "Prethodna sličica"),
    ("next_frame", "Sljedeća sličica"),
    ("fps", "FPS"),
    ("zoom", "Zumiranje"),
    // Status line
    ("pose_mode", "Način poze."),
    (
        "status_ready",
        "Povucite zglob za pozu. Ruke i stopala koriste IK; povucite kukove za pomicanje.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static HU: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Lejátszás"),
    ("pause", "Szünet"),
    ("add_frame", "+ Képkocka"),
    ("duplicate_frame", "Duplikálás"),
    ("delete_frame", "Törlés"),
    ("inbetween", "Köztes képkocka"),
    ("copy_frame", "Képkocka másolása"),
    ("paste_frame", "Képkocka beillesztése"),
    // History
    ("undo", "Visszavonás"),
    ("redo", "Újra"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Szegmens"),
    ("delete_segment", "- Szegmens"),
    ("len_plus", "Hossz +"),
    ("len_minus", "Hossz -"),
    ("thick_plus", "Vastagság +"),
    ("thick_minus", "Vastagság -"),
    ("shape", "Alak"),
    // Canvas modes and drawing
    ("onion_skin", "Hagymahéj"),
    ("draw", "Rajzolás"),
    ("clear_drawing", "Rajz törlése"),
    ("camera_mode", "Kameramód"),
    ("cam_zoom_plus", "Kamerazoom +"),
    ("cam_zoom_minus", "Kamerazoom -"),
    ("follow_cam", "Kamera követése"),
    ("tween_play", "Tween lejátszás"),
    // Particle effects
    ("fx_burst", "FX Robbanás"),
    ("fx_sparkle", "FX Szikrák"),
    ("fx_smoke", "FX Füst"),
    ("clear_fx", "FX törlése"),
    // Files and export
    ("save", "Mentés"),
    ("load", "Betöltés"),
    ("export_png", "PNG exportálása"),
    ("export_gif", "GIF exportálása"),
    ("export_mp4", "MP4 exportálása"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", ".piv importálása"),
    ("import_stk", ".stk importálása"),
    ("export_piv", ".piv exportálása"),
    ("export_stk", ".stk exportálása"),
    // Audio and movieclips
    ("add_audio", "Hang hozzáadása"),
    ("make_clip", "Klip készítése"),
    ("insert_clip", "Klip beszúrása"),
    // Themes and panels
    ("light_theme", "Világos téma"),
    ("dark_theme", "Sötét téma"),
    ("figures_layers", "FIGURÁK (rétegek)"),
    // Panels and help (Wave 9 UI)
    ("help", "Súgó"),
    ("inspector", "Vizsgáló"),
    ("easing", "Easing"),
    ("colour_picker", "Színválasztó"),
    ("export_centre", "Exportközpont"),
    ("dopesheet", "Expozíciós lap"),
    ("hud_loop", "Ismétlés"),
    ("transport_first", "Első képkocka"),
    ("transport_last", "Utolsó képkocka"),
    ("fx_panel", "FX-panel"),
    ("clips_panel", "Klippanel"),
    ("weather_panel", "Időjáráspanel"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Beállítások"),
    ("frame", "Képkocka"),
    ("frames", "Képkockák"),
    ("figures", "Figurák"),
    ("prev_frame", "Előző képkocka"),
    ("next_frame", "Következő képkocka"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Póz mód."),
    (
        "status_ready",
        "Húzzon egy ízületet a pózhoz. A kezek és lábak IK-t használnak; a mozgatáshoz húzza a csípőt.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static ID: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Putar"),
    ("pause", "Jeda"),
    ("add_frame", "+ Bingkai"),
    ("duplicate_frame", "Gandakan"),
    ("delete_frame", "Hapus"),
    ("inbetween", "Bingkai antara"),
    ("copy_frame", "Salin bingkai"),
    ("paste_frame", "Tempel bingkai"),
    // History
    ("undo", "Urungkan"),
    ("redo", "Ulangi"),
    // Figure building
    ("add_figure", "+ Figur"),
    ("add_segment", "+ Segmen"),
    ("delete_segment", "- Segmen"),
    ("len_plus", "Panjang +"),
    ("len_minus", "Panjang -"),
    ("thick_plus", "Tebal +"),
    ("thick_minus", "Tebal -"),
    ("shape", "Bentuk"),
    // Canvas modes and drawing
    ("onion_skin", "Kulit bawang"),
    ("draw", "Gambar"),
    ("clear_drawing", "Hapus gambar"),
    ("camera_mode", "Mode kamera"),
    ("cam_zoom_plus", "Zoom kamera +"),
    ("cam_zoom_minus", "Zoom kamera -"),
    ("follow_cam", "Ikuti kamera"),
    ("tween_play", "Putar tween"),
    // Particle effects
    ("fx_burst", "FX Ledakan"),
    ("fx_sparkle", "FX Kilau"),
    ("fx_smoke", "FX Asap"),
    ("clear_fx", "Hapus FX"),
    // Files and export
    ("save", "Simpan"),
    ("load", "Muat"),
    ("export_png", "Ekspor PNG"),
    ("export_gif", "Ekspor GIF"),
    ("export_mp4", "Ekspor MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Lembar sprite"),
    ("import_piv", "Impor .piv"),
    ("import_stk", "Impor .stk"),
    ("export_piv", "Ekspor .piv"),
    ("export_stk", "Ekspor .stk"),
    // Audio and movieclips
    ("add_audio", "Tambah audio"),
    ("make_clip", "Buat klip"),
    ("insert_clip", "Sisipkan klip"),
    // Themes and panels
    ("light_theme", "Tema terang"),
    ("dark_theme", "Tema gelap"),
    ("figures_layers", "FIGUR (lapisan)"),
    // Panels and help (Wave 9 UI)
    ("help", "Bantuan"),
    ("inspector", "Inspektur"),
    ("easing", "Easing"),
    ("colour_picker", "Pemilih warna"),
    ("export_centre", "Pusat ekspor"),
    ("dopesheet", "Lembar dope"),
    ("hud_loop", "Perulangan"),
    ("transport_first", "Bingkai pertama"),
    ("transport_last", "Bingkai terakhir"),
    ("fx_panel", "Panel FX"),
    ("clips_panel", "Panel klip"),
    ("weather_panel", "Panel cuaca"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Pengaturan"),
    ("frame", "Bingkai"),
    ("frames", "Bingkai"),
    ("figures", "Figur"),
    ("prev_frame", "Bingkai sebelumnya"),
    ("next_frame", "Bingkai berikutnya"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Mode pose."),
    (
        "status_ready",
        "Seret sendi untuk berpose. Tangan dan kaki menggunakan IK; seret pinggul untuk memindahkan.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static IT: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Riproduci"),
    ("pause", "Pausa"),
    ("add_frame", "+ Fotogramma"),
    ("duplicate_frame", "Duplica"),
    ("delete_frame", "Elimina"),
    ("inbetween", "Fotogramma intermedio"),
    ("copy_frame", "Copia fotogramma"),
    ("paste_frame", "Incolla fotogramma"),
    // History
    ("undo", "Annulla"),
    ("redo", "Ripeti"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segmento"),
    ("delete_segment", "- Segmento"),
    ("len_plus", "Lungh. +"),
    ("len_minus", "Lungh. -"),
    ("thick_plus", "Spessore +"),
    ("thick_minus", "Spessore -"),
    ("shape", "Forma"),
    // Canvas modes and drawing
    ("onion_skin", "Buccia di cipolla"),
    ("draw", "Disegna"),
    ("clear_drawing", "Cancella disegno"),
    ("camera_mode", "Modalità fotocamera"),
    ("cam_zoom_plus", "Zoom fotocamera +"),
    ("cam_zoom_minus", "Zoom fotocamera -"),
    ("follow_cam", "Segui fotocamera"),
    ("tween_play", "Interpolazione"),
    // Particle effects
    ("fx_burst", "FX Esplosione"),
    ("fx_sparkle", "FX Scintille"),
    ("fx_smoke", "FX Fumo"),
    ("clear_fx", "Cancella FX"),
    // Files and export
    ("save", "Salva"),
    ("load", "Carica"),
    ("export_png", "Esporta PNG"),
    ("export_gif", "Esporta GIF"),
    ("export_mp4", "Esporta MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Foglio sprite"),
    ("import_piv", "Importa .piv"),
    ("import_stk", "Importa .stk"),
    ("export_piv", "Esporta .piv"),
    ("export_stk", "Esporta .stk"),
    // Audio and movieclips
    ("add_audio", "Aggiungi audio"),
    ("make_clip", "Crea clip"),
    ("insert_clip", "Inserisci clip"),
    // Themes and panels
    ("light_theme", "Tema chiaro"),
    ("dark_theme", "Tema scuro"),
    ("figures_layers", "FIGURE (livelli)"),
    // Panels and help (Wave 9 UI)
    ("help", "Aiuto"),
    ("inspector", "Ispettore"),
    ("easing", "Easing"),
    ("colour_picker", "Selettore colore"),
    ("export_centre", "Centro di esportazione"),
    ("dopesheet", "Foglio di esposizione"),
    ("hud_loop", "Ciclo"),
    ("transport_first", "Primo fotogramma"),
    ("transport_last", "Ultimo fotogramma"),
    ("fx_panel", "Pannello FX"),
    ("clips_panel", "Pannello clip"),
    ("weather_panel", "Pannello meteo"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Anima"),
    ("ws_draw", "Disegna"),
    ("ws_build", "Costruisci"),
    ("ws_graph", "Grafico"),
    ("ws_effects", "Effetti"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Esporta"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Impostazioni"),
    ("frame", "Fotogramma"),
    ("frames", "Fotogrammi"),
    ("figures", "Figure"),
    ("prev_frame", "Fotogramma precedente"),
    ("next_frame", "Fotogramma successivo"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Modalità posa."),
    (
        "status_ready",
        "Trascina un'articolazione per la posa. Mani e piedi usano IK; trascina i fianchi per spostare.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static KK: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Ойнату"),
    ("pause", "Кідірту"),
    ("add_frame", "+ Кадр"),
    ("duplicate_frame", "Қайталау"),
    ("delete_frame", "Жою"),
    ("inbetween", "Аралық кадр"),
    ("copy_frame", "Кадрды көшіру"),
    ("paste_frame", "Кадрды қою"),
    // History
    ("undo", "Болдырмау"),
    ("redo", "Қайталау"),
    // Figure building
    ("add_figure", "+ Фигура"),
    ("add_segment", "+ Сегмент"),
    ("delete_segment", "- Сегмент"),
    ("len_plus", "Ұзындық +"),
    ("len_minus", "Ұзындық -"),
    ("thick_plus", "Қалыңдық +"),
    ("thick_minus", "Қалыңдық -"),
    ("shape", "Пішін"),
    // Canvas modes and drawing
    ("onion_skin", "Пияз қабығы"),
    ("draw", "Сурет салу"),
    ("clear_drawing", "Суретті тазалау"),
    ("camera_mode", "Камера режимі"),
    ("cam_zoom_plus", "Камера масштабы +"),
    ("cam_zoom_minus", "Камера масштабы -"),
    ("follow_cam", "Камераны бақылау"),
    ("tween_play", "Твин ойнату"),
    // Particle effects
    ("fx_burst", "FX Жарылыс"),
    ("fx_sparkle", "FX Жарқыл"),
    ("fx_smoke", "FX Түтін"),
    ("clear_fx", "FX тазалау"),
    // Files and export
    ("save", "Сақтау"),
    ("load", "Жүктеу"),
    ("export_png", "PNG экспорт"),
    ("export_gif", "GIF экспорт"),
    ("export_mp4", "MP4 экспорт"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Спраит парағы"),
    ("import_piv", ".piv импорт"),
    ("import_stk", ".stk импорт"),
    ("export_piv", ".piv экспорт"),
    ("export_stk", ".stk экспорт"),
    // Audio and movieclips
    ("add_audio", "Аудио қосу"),
    ("make_clip", "Клип жасау"),
    ("insert_clip", "Клип кірістіру"),
    // Themes and panels
    ("light_theme", "Ашық тақырып"),
    ("dark_theme", "Қараңғы тақырып"),
    ("figures_layers", "ФИГУРАЛАР (қабаттар)"),
    // Panels and help (Wave 9 UI)
    ("help", "Анықтама"),
    ("inspector", "Инспектор"),
    ("easing", "Жұмсарту"),
    ("colour_picker", "Түс таңдау"),
    ("export_centre", "Экспорт орталығы"),
    ("dopesheet", "Экспозиция парағы"),
    ("hud_loop", "Цикл"),
    ("transport_first", "Бірінші кадр"),
    ("transport_last", "Соңғы кадр"),
    ("fx_panel", "FX панелі"),
    ("clips_panel", "Клиптер панелі"),
    ("weather_panel", "Ауа райы панелі"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Параметрлер"),
    ("frame", "Кадр"),
    ("frames", "Кадрлар"),
    ("figures", "Фигуралар"),
    ("prev_frame", "Алдыңғы кадр"),
    ("next_frame", "Келесі кадр"),
    ("fps", "FPS"),
    ("zoom", "Масштаб"),
    // Status line
    ("pose_mode", "Поза режимі."),
    (
        "status_ready",
        "Поза қою үшін буынды сүйреңіз. Қолдар мен аяқтар IK пайдаланады; жылжыту үшін жамбасты сүйреңіз.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static NB: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Spill av"),
    ("pause", "Pause"),
    ("add_frame", "+ Bilde"),
    ("duplicate_frame", "Dupliser"),
    ("delete_frame", "Slett"),
    ("inbetween", "Mellombilde"),
    ("copy_frame", "Kopier bilde"),
    ("paste_frame", "Lim inn bilde"),
    // History
    ("undo", "Angre"),
    ("redo", "Gjør om"),
    // Figure building
    ("add_figure", "+ Figur"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Lengde +"),
    ("len_minus", "Lengde -"),
    ("thick_plus", "Tykkelse +"),
    ("thick_minus", "Tykkelse -"),
    ("shape", "Form"),
    // Canvas modes and drawing
    ("onion_skin", "Løkskall"),
    ("draw", "Tegn"),
    ("clear_drawing", "Tøm tegning"),
    ("camera_mode", "Kameramodus"),
    ("cam_zoom_plus", "Kamerazoom +"),
    ("cam_zoom_minus", "Kamerazoom -"),
    ("follow_cam", "Følg kamera"),
    ("tween_play", "Interpolering"),
    // Particle effects
    ("fx_burst", "FX Eksplosjon"),
    ("fx_sparkle", "FX Gnister"),
    ("fx_smoke", "FX Røyk"),
    ("clear_fx", "Tøm FX"),
    // Files and export
    ("save", "Lagre"),
    ("load", "Last inn"),
    ("export_png", "Eksporter PNG"),
    ("export_gif", "Eksporter GIF"),
    ("export_mp4", "Eksporter MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", "Importer .piv"),
    ("import_stk", "Importer .stk"),
    ("export_piv", "Eksporter .piv"),
    ("export_stk", "Eksporter .stk"),
    // Audio and movieclips
    ("add_audio", "Legg til lyd"),
    ("make_clip", "Lag klipp"),
    ("insert_clip", "Sett inn klipp"),
    // Themes and panels
    ("light_theme", "Lyst tema"),
    ("dark_theme", "Mørkt tema"),
    ("figures_layers", "FIGURER (lag)"),
    // Panels and help (Wave 9 UI)
    ("help", "Hjelp"),
    ("inspector", "Inspektør"),
    ("easing", "Easing"),
    ("colour_picker", "Fargevelger"),
    ("export_centre", "Eksportsenter"),
    ("dopesheet", "Eksponeringsark"),
    ("hud_loop", "Løkke"),
    ("transport_first", "Første bilde"),
    ("transport_last", "Siste bilde"),
    ("fx_panel", "FX-panel"),
    ("clips_panel", "Klipppanel"),
    ("weather_panel", "Værpanel"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Innstillinger"),
    ("frame", "Bilde"),
    ("frames", "Bilder"),
    ("figures", "Figurer"),
    ("prev_frame", "Forrige bilde"),
    ("next_frame", "Neste bilde"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Poseringsmodus."),
    (
        "status_ready",
        "Dra et ledd for å posere. Hender og føtter bruker IK; dra hoftene for å flytte.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static NL: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Afspelen"),
    ("pause", "Pauzeren"),
    ("add_frame", "+ Frame"),
    ("duplicate_frame", "Dupliceren"),
    ("delete_frame", "Verwijderen"),
    ("inbetween", "Tussenframe"),
    ("copy_frame", "Frame kopiëren"),
    ("paste_frame", "Frame plakken"),
    // History
    ("undo", "Ongedaan maken"),
    ("redo", "Opnieuw"),
    // Figure building
    ("add_figure", "+ Figuur"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Lengte +"),
    ("len_minus", "Lengte -"),
    ("thick_plus", "Dikte +"),
    ("thick_minus", "Dikte -"),
    ("shape", "Vorm"),
    // Canvas modes and drawing
    ("onion_skin", "Uienschil"),
    ("draw", "Tekenen"),
    ("clear_drawing", "Tekening wissen"),
    ("camera_mode", "Cameramodus"),
    ("cam_zoom_plus", "Camerazoom +"),
    ("cam_zoom_minus", "Camerazoom -"),
    ("follow_cam", "Camera volgen"),
    ("tween_play", "Tween afspelen"),
    // Particle effects
    ("fx_burst", "FX Explosie"),
    ("fx_sparkle", "FX Vonken"),
    ("fx_smoke", "FX Rook"),
    ("clear_fx", "FX wissen"),
    // Files and export
    ("save", "Opslaan"),
    ("load", "Laden"),
    ("export_png", "PNG exporteren"),
    ("export_gif", "GIF exporteren"),
    ("export_mp4", "MP4 exporteren"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", ".piv importeren"),
    ("import_stk", ".stk importeren"),
    ("export_piv", ".piv exporteren"),
    ("export_stk", ".stk exporteren"),
    // Audio and movieclips
    ("add_audio", "Audio toevoegen"),
    ("make_clip", "Clip maken"),
    ("insert_clip", "Clip invoegen"),
    // Themes and panels
    ("light_theme", "Licht thema"),
    ("dark_theme", "Donker thema"),
    ("figures_layers", "FIGUREN (lagen)"),
    // Panels and help (Wave 9 UI)
    ("help", "Help"),
    ("inspector", "Inspecteur"),
    ("easing", "Easing"),
    ("colour_picker", "Kleurkiezer"),
    ("export_centre", "Exportcentrum"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Herhalen"),
    ("transport_first", "Eerste Frame"),
    ("transport_last", "Laatste Frame"),
    ("fx_panel", "FX-paneel"),
    ("clips_panel", "Clipspaneel"),
    ("weather_panel", "Weerpaneel"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Instellingen"),
    ("frame", "Frame"),
    ("frames", "Frames"),
    ("figures", "Figuren"),
    ("prev_frame", "Vorige frame"),
    ("next_frame", "Volgende frame"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Posemodus."),
    (
        "status_ready",
        "Sleep een gewricht om te poseren. Handen en voeten gebruiken IK; sleep de heupen om te verplaatsen.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static PL: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Odtwórz"),
    ("pause", "Pauza"),
    ("add_frame", "+ Klatka"),
    ("duplicate_frame", "Duplikuj"),
    ("delete_frame", "Usuń"),
    ("inbetween", "Klatka pośrednia"),
    ("copy_frame", "Kopiuj klatkę"),
    ("paste_frame", "Wklej klatkę"),
    // History
    ("undo", "Cofnij"),
    ("redo", "Ponów"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Długość +"),
    ("len_minus", "Długość -"),
    ("thick_plus", "Grubość +"),
    ("thick_minus", "Grubość -"),
    ("shape", "Kształt"),
    // Canvas modes and drawing
    ("onion_skin", "Skórka cebuli"),
    ("draw", "Rysuj"),
    ("clear_drawing", "Wyczyść rysunek"),
    ("camera_mode", "Tryb kamery"),
    ("cam_zoom_plus", "Zoom kamery +"),
    ("cam_zoom_minus", "Zoom kamery -"),
    ("follow_cam", "Podążaj za kamerą"),
    ("tween_play", "Odtwórz tween"),
    // Particle effects
    ("fx_burst", "FX Wybuch"),
    ("fx_sparkle", "FX Iskry"),
    ("fx_smoke", "FX Dym"),
    ("clear_fx", "Wyczyść FX"),
    // Files and export
    ("save", "Zapisz"),
    ("load", "Wczytaj"),
    ("export_png", "Eksport PNG"),
    ("export_gif", "Eksport GIF"),
    ("export_mp4", "Eksport MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Arkusz sprite'ów"),
    ("import_piv", "Import .piv"),
    ("import_stk", "Import .stk"),
    ("export_piv", "Eksport .piv"),
    ("export_stk", "Eksport .stk"),
    // Audio and movieclips
    ("add_audio", "Dodaj dźwięk"),
    ("make_clip", "Utwórz klip"),
    ("insert_clip", "Wstaw klip"),
    // Themes and panels
    ("light_theme", "Jasny motyw"),
    ("dark_theme", "Ciemny motyw"),
    ("figures_layers", "FIGURY (warstwy)"),
    // Panels and help (Wave 9 UI)
    ("help", "Pomoc"),
    ("inspector", "Inspektor"),
    ("easing", "Easing"),
    ("colour_picker", "Próbnik kolorów"),
    ("export_centre", "Centrum eksportu"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Pętla"),
    ("transport_first", "Pierwsza klatka"),
    ("transport_last", "Ostatnia klatka"),
    ("fx_panel", "Panel FX"),
    ("clips_panel", "Panel klipów"),
    ("weather_panel", "Panel pogody"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Ustawienia"),
    ("frame", "Klatka"),
    ("frames", "Klatki"),
    ("figures", "Figury"),
    ("prev_frame", "Poprzednia klatka"),
    ("next_frame", "Następna klatka"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Tryb pozy."),
    (
        "status_ready",
        "Przeciągnij staw, aby ustawić pozę. Dłonie i stopy używają IK; przeciągnij biodra, aby przenieść.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static PT_BR: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Reproduzir"),
    ("pause", "Pausar"),
    ("add_frame", "+ Quadro"),
    ("duplicate_frame", "Duplicar"),
    ("delete_frame", "Excluir"),
    ("inbetween", "Quadro intermediário"),
    ("copy_frame", "Copiar quadro"),
    ("paste_frame", "Colar quadro"),
    // History
    ("undo", "Desfazer"),
    ("redo", "Refazer"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segmento"),
    ("delete_segment", "- Segmento"),
    ("len_plus", "Compr. +"),
    ("len_minus", "Compr. -"),
    ("thick_plus", "Espessura +"),
    ("thick_minus", "Espessura -"),
    ("shape", "Forma"),
    // Canvas modes and drawing
    ("onion_skin", "Casca de cebola"),
    ("draw", "Desenhar"),
    ("clear_drawing", "Limpar desenho"),
    ("camera_mode", "Modo câmera"),
    ("cam_zoom_plus", "Zoom da câmera +"),
    ("cam_zoom_minus", "Zoom da câmera -"),
    ("follow_cam", "Seguir câmera"),
    ("tween_play", "Interpolação"),
    // Particle effects
    ("fx_burst", "FX Explosão"),
    ("fx_sparkle", "FX Faíscas"),
    ("fx_smoke", "FX Fumaça"),
    ("clear_fx", "Limpar FX"),
    // Files and export
    ("save", "Salvar"),
    ("load", "Carregar"),
    ("export_png", "Exportar PNG"),
    ("export_gif", "Exportar GIF"),
    ("export_mp4", "Exportar MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Folha de sprites"),
    ("import_piv", "Importar .piv"),
    ("import_stk", "Importar .stk"),
    ("export_piv", "Exportar .piv"),
    ("export_stk", "Exportar .stk"),
    // Audio and movieclips
    ("add_audio", "Adicionar áudio"),
    ("make_clip", "Criar clipe"),
    ("insert_clip", "Inserir clipe"),
    // Themes and panels
    ("light_theme", "Tema claro"),
    ("dark_theme", "Tema escuro"),
    ("figures_layers", "FIGURAS (camadas)"),
    // Panels and help (Wave 9 UI)
    ("help", "Ajuda"),
    ("inspector", "Inspetor"),
    ("easing", "Suavização"),
    ("colour_picker", "Seletor de cor"),
    ("export_centre", "Centro de exportação"),
    ("dopesheet", "Folha de exposição"),
    ("hud_loop", "Repetição"),
    ("transport_first", "Primeiro quadro"),
    ("transport_last", "Último quadro"),
    ("fx_panel", "Painel de FX"),
    ("clips_panel", "Painel de clipes"),
    ("weather_panel", "Painel de clima"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animar"),
    ("ws_draw", "Desenhar"),
    ("ws_build", "Construir"),
    ("ws_graph", "Gráfico"),
    ("ws_effects", "Efeitos"),
    ("ws_audio", "Áudio"),
    ("ws_3d", "3D"),
    ("ws_export", "Exportar"),
    ("ws_home", "Início"),
    // Rune shell labels (Phase E)
    ("settings", "Configurações"),
    ("frame", "Quadro"),
    ("frames", "Quadros"),
    ("figures", "Figuras"),
    ("prev_frame", "Quadro anterior"),
    ("next_frame", "Quadro seguinte"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Modo de pose."),
    (
        "status_ready",
        "Arraste uma articulação para posar. Mãos e pés usam IK; arraste os quadris para mover.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static RO: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Redare"),
    ("pause", "Pauză"),
    ("add_frame", "+ Cadru"),
    ("duplicate_frame", "Duplicare"),
    ("delete_frame", "Ștergere"),
    ("inbetween", "Cadru intermediar"),
    ("copy_frame", "Copiere cadru"),
    ("paste_frame", "Lipire cadru"),
    // History
    ("undo", "Anulare"),
    ("redo", "Refacere"),
    // Figure building
    ("add_figure", "+ Figură"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Lungime +"),
    ("len_minus", "Lungime -"),
    ("thick_plus", "Grosime +"),
    ("thick_minus", "Grosime -"),
    ("shape", "Formă"),
    // Canvas modes and drawing
    ("onion_skin", "Coajă de ceapă"),
    ("draw", "Desenare"),
    ("clear_drawing", "Șterge desenul"),
    ("camera_mode", "Mod cameră"),
    ("cam_zoom_plus", "Zoom cameră +"),
    ("cam_zoom_minus", "Zoom cameră -"),
    ("follow_cam", "Urmărește camera"),
    ("tween_play", "Redare tween"),
    // Particle effects
    ("fx_burst", "FX Explozie"),
    ("fx_sparkle", "FX Scântei"),
    ("fx_smoke", "FX Fum"),
    ("clear_fx", "Șterge FX"),
    // Files and export
    ("save", "Salvare"),
    ("load", "Încărcare"),
    ("export_png", "Export PNG"),
    ("export_gif", "Export GIF"),
    ("export_mp4", "Export MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Foaie de sprite-uri"),
    ("import_piv", "Import .piv"),
    ("import_stk", "Import .stk"),
    ("export_piv", "Export .piv"),
    ("export_stk", "Export .stk"),
    // Audio and movieclips
    ("add_audio", "Adaugă audio"),
    ("make_clip", "Creează clip"),
    ("insert_clip", "Inserează clip"),
    // Themes and panels
    ("light_theme", "Temă deschisă"),
    ("dark_theme", "Temă întunecată"),
    ("figures_layers", "FIGURI (straturi)"),
    // Panels and help (Wave 9 UI)
    ("help", "Ajutor"),
    ("inspector", "Inspector"),
    ("easing", "Easing"),
    ("colour_picker", "Selector de culoare"),
    ("export_centre", "Centru de export"),
    ("dopesheet", "Foaie de expunere"),
    ("hud_loop", "Buclă"),
    ("transport_first", "Primul cadru"),
    ("transport_last", "Ultimul cadru"),
    ("fx_panel", "Panou FX"),
    ("clips_panel", "Panou clipuri"),
    ("weather_panel", "Panou vreme"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Setări"),
    ("frame", "Cadru"),
    ("frames", "Cadre"),
    ("figures", "Figuri"),
    ("prev_frame", "Cadrul anterior"),
    ("next_frame", "Cadrul următor"),
    ("fps", "FPS"),
    ("zoom", "Zoom"),
    // Status line
    ("pose_mode", "Mod poză."),
    (
        "status_ready",
        "Trageți o articulație pentru poză. Mâinile și picioarele folosesc IK; trageți șoldurile pentru a muta.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static RU: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Воспроизвести"),
    ("pause", "Пауза"),
    ("add_frame", "+ Кадр"),
    ("duplicate_frame", "Дублировать"),
    ("delete_frame", "Удалить"),
    ("inbetween", "Промежуточный кадр"),
    ("copy_frame", "Копировать кадр"),
    ("paste_frame", "Вставить кадр"),
    // History
    ("undo", "Отменить"),
    ("redo", "Повторить"),
    // Figure building
    ("add_figure", "+ Фигура"),
    ("add_segment", "+ Сегмент"),
    ("delete_segment", "- Сегмент"),
    ("len_plus", "Длина +"),
    ("len_minus", "Длина -"),
    ("thick_plus", "Толщина +"),
    ("thick_minus", "Толщина -"),
    ("shape", "Форма"),
    // Canvas modes and drawing
    ("onion_skin", "Луковая шелуха"),
    ("draw", "Рисовать"),
    ("clear_drawing", "Очистить рисунок"),
    ("camera_mode", "Режим камеры"),
    ("cam_zoom_plus", "Масштаб камеры +"),
    ("cam_zoom_minus", "Масштаб камеры -"),
    ("follow_cam", "Следовать за камерой"),
    ("tween_play", "Твининг"),
    // Particle effects
    ("fx_burst", "FX Взрыв"),
    ("fx_sparkle", "FX Искры"),
    ("fx_smoke", "FX Дым"),
    ("clear_fx", "Очистить FX"),
    // Files and export
    ("save", "Сохранить"),
    ("load", "Загрузить"),
    ("export_png", "Экспорт PNG"),
    ("export_gif", "Экспорт GIF"),
    ("export_mp4", "Экспорт MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Спрайтшит"),
    ("import_piv", "Импорт .piv"),
    ("import_stk", "Импорт .stk"),
    ("export_piv", "Экспорт .piv"),
    ("export_stk", "Экспорт .stk"),
    // Audio and movieclips
    ("add_audio", "Добавить аудио"),
    ("make_clip", "Создать клип"),
    ("insert_clip", "Вставить клип"),
    // Themes and panels
    ("light_theme", "Светлая тема"),
    ("dark_theme", "Тёмная тема"),
    ("figures_layers", "ФИГУРЫ (слои)"),
    // Panels and help (Wave 9 UI)
    ("help", "Справка"),
    ("inspector", "Инспектор"),
    ("easing", "Смягчение"),
    ("colour_picker", "Выбор цвета"),
    ("export_centre", "Центр экспорта"),
    ("dopesheet", "Экспозиционный лист"),
    ("hud_loop", "Зацикливание"),
    ("transport_first", "Первый кадр"),
    ("transport_last", "Последний кадр"),
    ("fx_panel", "Панель FX"),
    ("clips_panel", "Панель клипов"),
    ("weather_panel", "Панель погоды"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Анимация"),
    ("ws_draw", "Рисование"),
    ("ws_build", "Сборка"),
    ("ws_graph", "График"),
    ("ws_effects", "Эффекты"),
    ("ws_audio", "Аудио"),
    ("ws_3d", "3D"),
    ("ws_export", "Экспорт"),
    ("ws_home", "Главная"),
    // Rune shell labels (Phase E)
    ("settings", "Настройки"),
    ("frame", "Кадр"),
    ("frames", "Кадры"),
    ("figures", "Фигуры"),
    ("prev_frame", "Предыдущий кадр"),
    ("next_frame", "Следующий кадр"),
    ("fps", "FPS"),
    ("zoom", "Масштаб"),
    // Status line
    ("pose_mode", "Режим позы."),
    (
        "status_ready",
        "Перетащите сустав, чтобы задать позу. Руки и ноги используют IK; перетащите бёдра для перемещения.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static SL: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Predvajaj"),
    ("pause", "Premor"),
    ("add_frame", "+ Sličica"),
    ("duplicate_frame", "Podvoji"),
    ("delete_frame", "Izbriši"),
    ("inbetween", "Vmesna sličica"),
    ("copy_frame", "Kopiraj sličico"),
    ("paste_frame", "Prilepi sličico"),
    // History
    ("undo", "Razveljavi"),
    ("redo", "Uveljavi"),
    // Figure building
    ("add_figure", "+ Figura"),
    ("add_segment", "+ Segment"),
    ("delete_segment", "- Segment"),
    ("len_plus", "Dolžina +"),
    ("len_minus", "Dolžina -"),
    ("thick_plus", "Debelina +"),
    ("thick_minus", "Debelina -"),
    ("shape", "Oblika"),
    // Canvas modes and drawing
    ("onion_skin", "Čebulna lupina"),
    ("draw", "Riši"),
    ("clear_drawing", "Počisti risbo"),
    ("camera_mode", "Način kamere"),
    ("cam_zoom_plus", "Povečava kamere +"),
    ("cam_zoom_minus", "Povečava kamere -"),
    ("follow_cam", "Sledi kameri"),
    ("tween_play", "Interpolacija"),
    // Particle effects
    ("fx_burst", "FX Eksplozija"),
    ("fx_sparkle", "FX Iskre"),
    ("fx_smoke", "FX Dim"),
    ("clear_fx", "Počisti FX"),
    // Files and export
    ("save", "Shrani"),
    ("load", "Naloži"),
    ("export_png", "Izvoz PNG"),
    ("export_gif", "Izvoz GIF"),
    ("export_mp4", "Izvoz MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Spritesheet"),
    ("import_piv", "Uvoz .piv"),
    ("import_stk", "Uvoz .stk"),
    ("export_piv", "Izvoz .piv"),
    ("export_stk", "Izvoz .stk"),
    // Audio and movieclips
    ("add_audio", "Dodaj zvok"),
    ("make_clip", "Ustvari posnetek"),
    ("insert_clip", "Vstavi posnetek"),
    // Themes and panels
    ("light_theme", "Svetla tema"),
    ("dark_theme", "Temna tema"),
    ("figures_layers", "FIGURE (plasti)"),
    // Panels and help (Wave 9 UI)
    ("help", "Pomoč"),
    ("inspector", "Inšpektor"),
    ("easing", "Easing"),
    ("colour_picker", "Izbirnik barv"),
    ("export_centre", "Center za izvoz"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Zanka"),
    ("transport_first", "Prva sličica"),
    ("transport_last", "Zadnja sličica"),
    ("fx_panel", "Plošča FX"),
    ("clips_panel", "Plošča posnetkov"),
    ("weather_panel", "Plošča vremena"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Nastavitve"),
    ("frame", "Sličica"),
    ("frames", "Sličice"),
    ("figures", "Figure"),
    ("prev_frame", "Prejšnja sličica"),
    ("next_frame", "Naslednja sličica"),
    ("fps", "FPS"),
    ("zoom", "Povečava"),
    // Status line
    ("pose_mode", "Način poze."),
    (
        "status_ready",
        "Povlecite sklep za pozo. Roke in stopala uporabljajo IK; povlecite boke za premik.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static TR: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Oynat"),
    ("pause", "Duraklat"),
    ("add_frame", "+ Kare"),
    ("duplicate_frame", "Çoğalt"),
    ("delete_frame", "Sil"),
    ("inbetween", "Ara kare"),
    ("copy_frame", "Kareyi kopyala"),
    ("paste_frame", "Kareyi yapıştır"),
    // History
    ("undo", "Geri al"),
    ("redo", "Yinele"),
    // Figure building
    ("add_figure", "+ Figür"),
    ("add_segment", "+ Parça"),
    ("delete_segment", "- Parça"),
    ("len_plus", "Uzunluk +"),
    ("len_minus", "Uzunluk -"),
    ("thick_plus", "Kalınlık +"),
    ("thick_minus", "Kalınlık -"),
    ("shape", "Şekil"),
    // Canvas modes and drawing
    ("onion_skin", "Soğan kabuğu"),
    ("draw", "Çiz"),
    ("clear_drawing", "Çizimi temizle"),
    ("camera_mode", "Kamera modu"),
    ("cam_zoom_plus", "Kamera yakınlaştırma +"),
    ("cam_zoom_minus", "Kamera yakınlaştırma -"),
    ("follow_cam", "Kamerayı takip et"),
    ("tween_play", "Tween oynat"),
    // Particle effects
    ("fx_burst", "FX Patlama"),
    ("fx_sparkle", "FX Kıvılcım"),
    ("fx_smoke", "FX Duman"),
    ("clear_fx", "FX temizle"),
    // Files and export
    ("save", "Kaydet"),
    ("load", "Yükle"),
    ("export_png", "PNG dışa aktar"),
    ("export_gif", "GIF dışa aktar"),
    ("export_mp4", "MP4 dışa aktar"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Sprite sayfası"),
    ("import_piv", ".piv içe aktar"),
    ("import_stk", ".stk içe aktar"),
    ("export_piv", ".piv dışa aktar"),
    ("export_stk", ".stk dışa aktar"),
    // Audio and movieclips
    ("add_audio", "Ses ekle"),
    ("make_clip", "Klip oluştur"),
    ("insert_clip", "Klip ekle"),
    // Themes and panels
    ("light_theme", "Açık tema"),
    ("dark_theme", "Koyu tema"),
    ("figures_layers", "FİGÜRLER (katmanlar)"),
    // Panels and help (Wave 9 UI)
    ("help", "Yardım"),
    ("inspector", "Denetçi"),
    ("easing", "Yumuşatma"),
    ("colour_picker", "Renk seçici"),
    ("export_centre", "Dışa aktarma merkezi"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Döngü"),
    ("transport_first", "İlk kare"),
    ("transport_last", "Son kare"),
    ("fx_panel", "FX paneli"),
    ("clips_panel", "Klip paneli"),
    ("weather_panel", "Hava durumu paneli"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Ayarlar"),
    ("frame", "Kare"),
    ("frames", "Kareler"),
    ("figures", "Figürler"),
    ("prev_frame", "Önceki kare"),
    ("next_frame", "Sonraki kare"),
    ("fps", "FPS"),
    ("zoom", "Yakınlaştırma"),
    // Status line
    ("pose_mode", "Poz modu."),
    (
        "status_ready",
        "Poz vermek için bir eklemi sürükleyin. Eller ve ayaklar IK kullanır; taşımak için kalçayı sürükleyin.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static UK: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Відтворити"),
    ("pause", "Пауза"),
    ("add_frame", "+ Кадр"),
    ("duplicate_frame", "Дублювати"),
    ("delete_frame", "Видалити"),
    ("inbetween", "Проміжний кадр"),
    ("copy_frame", "Копіювати кадр"),
    ("paste_frame", "Вставити кадр"),
    // History
    ("undo", "Скасувати"),
    ("redo", "Повторити"),
    // Figure building
    ("add_figure", "+ Фігура"),
    ("add_segment", "+ Сегмент"),
    ("delete_segment", "- Сегмент"),
    ("len_plus", "Довжина +"),
    ("len_minus", "Довжина -"),
    ("thick_plus", "Товщина +"),
    ("thick_minus", "Товщина -"),
    ("shape", "Форма"),
    // Canvas modes and drawing
    ("onion_skin", "Цибулева шкірка"),
    ("draw", "Малювати"),
    ("clear_drawing", "Очистити малюнок"),
    ("camera_mode", "Режим камери"),
    ("cam_zoom_plus", "Масштаб камери +"),
    ("cam_zoom_minus", "Масштаб камери -"),
    ("follow_cam", "Стежити за камерою"),
    ("tween_play", "Твінінг"),
    // Particle effects
    ("fx_burst", "FX Вибух"),
    ("fx_sparkle", "FX Іскри"),
    ("fx_smoke", "FX Дим"),
    ("clear_fx", "Очистити FX"),
    // Files and export
    ("save", "Зберегти"),
    ("load", "Завантажити"),
    ("export_png", "Експорт PNG"),
    ("export_gif", "Експорт GIF"),
    ("export_mp4", "Експорт MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Спрайтшит"),
    ("import_piv", "Імпорт .piv"),
    ("import_stk", "Імпорт .stk"),
    ("export_piv", "Експорт .piv"),
    ("export_stk", "Експорт .stk"),
    // Audio and movieclips
    ("add_audio", "Додати аудіо"),
    ("make_clip", "Створити кліп"),
    ("insert_clip", "Вставити кліп"),
    // Themes and panels
    ("light_theme", "Світла тема"),
    ("dark_theme", "Темна тема"),
    ("figures_layers", "ФІГУРИ (шари)"),
    // Panels and help (Wave 9 UI)
    ("help", "Довідка"),
    ("inspector", "Інспектор"),
    ("easing", "Згладжування"),
    ("colour_picker", "Вибір кольору"),
    ("export_centre", "Центр експорту"),
    ("dopesheet", "Експозиційний лист"),
    ("hud_loop", "Зациклення"),
    ("transport_first", "Перший кадр"),
    ("transport_last", "Останній кадр"),
    ("fx_panel", "Панель FX"),
    ("clips_panel", "Панель кліпів"),
    ("weather_panel", "Панель погоди"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Налаштування"),
    ("frame", "Кадр"),
    ("frames", "Кадри"),
    ("figures", "Фігури"),
    ("prev_frame", "Попередній кадр"),
    ("next_frame", "Наступний кадр"),
    ("fps", "FPS"),
    ("zoom", "Масштаб"),
    // Status line
    ("pose_mode", "Режим пози."),
    (
        "status_ready",
        "Перетягніть суглоб, щоб задати позу. Руки й ноги використовують IK; перетягніть стегна для переміщення.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static VI: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "Phát"),
    ("pause", "Tạm dừng"),
    ("add_frame", "+ Khung hình"),
    ("duplicate_frame", "Nhân bản"),
    ("delete_frame", "Xóa"),
    ("inbetween", "Khung hình trung gian"),
    ("copy_frame", "Sao chép khung hình"),
    ("paste_frame", "Dán khung hình"),
    // History
    ("undo", "Hoàn tác"),
    ("redo", "Làm lại"),
    // Figure building
    ("add_figure", "+ Hình"),
    ("add_segment", "+ Đoạn"),
    ("delete_segment", "- Đoạn"),
    ("len_plus", "Dài +"),
    ("len_minus", "Dài -"),
    ("thick_plus", "Dày +"),
    ("thick_minus", "Dày -"),
    ("shape", "Hình dạng"),
    // Canvas modes and drawing
    ("onion_skin", "Vỏ hành"),
    ("draw", "Vẽ"),
    ("clear_drawing", "Xóa bản vẽ"),
    ("camera_mode", "Chế độ máy quay"),
    ("cam_zoom_plus", "Thu phóng máy quay +"),
    ("cam_zoom_minus", "Thu phóng máy quay -"),
    ("follow_cam", "Theo máy quay"),
    ("tween_play", "Phát tween"),
    // Particle effects
    ("fx_burst", "FX Nổ"),
    ("fx_sparkle", "FX Tia lửa"),
    ("fx_smoke", "FX Khói"),
    ("clear_fx", "Xóa FX"),
    // Files and export
    ("save", "Lưu"),
    ("load", "Tải"),
    ("export_png", "Xuất PNG"),
    ("export_gif", "Xuất GIF"),
    ("export_mp4", "Xuất MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "Bảng sprite"),
    ("import_piv", "Nhập .piv"),
    ("import_stk", "Nhập .stk"),
    ("export_piv", "Xuất .piv"),
    ("export_stk", "Xuất .stk"),
    // Audio and movieclips
    ("add_audio", "Thêm âm thanh"),
    ("make_clip", "Tạo clip"),
    ("insert_clip", "Chèn clip"),
    // Themes and panels
    ("light_theme", "Giao diện sáng"),
    ("dark_theme", "Giao diện tối"),
    ("figures_layers", "HÌNH (lớp)"),
    // Panels and help (Wave 9 UI)
    ("help", "Trợ giúp"),
    ("inspector", "Trình kiểm tra"),
    ("easing", "Easing"),
    ("colour_picker", "Bộ chọn màu"),
    ("export_centre", "Trung tâm xuất"),
    ("dopesheet", "Dopesheet"),
    ("hud_loop", "Lặp lại"),
    ("transport_first", "Khung hình đầu tiên"),
    ("transport_last", "Khung hình cuối cùng"),
    ("fx_panel", "Bảng FX"),
    ("clips_panel", "Bảng clip"),
    ("weather_panel", "Bảng thời tiết"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "Animate"),
    ("ws_draw", "Draw"),
    ("ws_build", "Build"),
    ("ws_graph", "Graph"),
    ("ws_effects", "Effects"),
    ("ws_audio", "Audio"),
    ("ws_3d", "3D"),
    ("ws_export", "Export"),
    ("ws_home", "Home"),
    // Rune shell labels (Phase E)
    ("settings", "Cài đặt"),
    ("frame", "Khung hình"),
    ("frames", "Khung hình"),
    ("figures", "Hình"),
    ("prev_frame", "Khung hình trước"),
    ("next_frame", "Khung hình tiếp theo"),
    ("fps", "FPS"),
    ("zoom", "Thu phóng"),
    // Status line
    ("pose_mode", "Chế độ tư thế."),
    (
        "status_ready",
        "Kéo một khớp để tạo tư thế. Tay và chân dùng IK; kéo hông để di chuyển.",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

static ZH_HANS: &[(&str, &str)] = &[
    // Playback and frames
    ("play", "播放"),
    ("pause", "暂停"),
    ("add_frame", "+ 帧"),
    ("duplicate_frame", "复制帧"),
    ("delete_frame", "删除帧"),
    ("inbetween", "中间帧"),
    ("copy_frame", "拷贝帧"),
    ("paste_frame", "粘贴帧"),
    // History
    ("undo", "撤销"),
    ("redo", "重做"),
    // Figure building
    ("add_figure", "+ 人物"),
    ("add_segment", "+ 段"),
    ("delete_segment", "- 段"),
    ("len_plus", "长度 +"),
    ("len_minus", "长度 -"),
    ("thick_plus", "粗细 +"),
    ("thick_minus", "粗细 -"),
    ("shape", "形状"),
    // Canvas modes and drawing
    ("onion_skin", "洋葱皮"),
    ("draw", "绘制"),
    ("clear_drawing", "清除绘制"),
    ("camera_mode", "摄像机模式"),
    ("cam_zoom_plus", "摄像机缩放 +"),
    ("cam_zoom_minus", "摄像机缩放 -"),
    ("follow_cam", "跟随摄像机"),
    ("tween_play", "补间播放"),
    // Particle effects
    ("fx_burst", "特效 爆炸"),
    ("fx_sparkle", "特效 火花"),
    ("fx_smoke", "特效 烟雾"),
    ("clear_fx", "清除特效"),
    // Files and export
    ("save", "保存"),
    ("load", "加载"),
    ("export_png", "导出 PNG"),
    ("export_gif", "导出 GIF"),
    ("export_mp4", "导出 MP4"),
    ("export_webm", "WebM"),
    ("export_svg", "SVG"),
    ("spritesheet", "精灵图集"),
    ("import_piv", "导入 .piv"),
    ("import_stk", "导入 .stk"),
    ("export_piv", "导出 .piv"),
    ("export_stk", "导出 .stk"),
    // Audio and movieclips
    ("add_audio", "添加音频"),
    ("make_clip", "创建片段"),
    ("insert_clip", "插入片段"),
    // Themes and panels
    ("light_theme", "浅色主题"),
    ("dark_theme", "深色主题"),
    ("figures_layers", "人物（图层）"),
    // Panels and help (Wave 9 UI)
    ("help", "帮助"),
    ("inspector", "检查器"),
    ("easing", "缓动"),
    ("colour_picker", "颜色选择器"),
    ("export_centre", "导出中心"),
    ("dopesheet", "摄影表"),
    ("hud_loop", "循环"),
    ("transport_first", "第一帧"),
    ("transport_last", "最后一帧"),
    ("fx_panel", "特效面板"),
    ("clips_panel", "片段面板"),
    ("weather_panel", "天气面板"),
    // Workspaces (Wave 14 redesign)
    ("ws_animate", "动画"),
    ("ws_draw", "绘制"),
    ("ws_build", "构建"),
    ("ws_graph", "图表"),
    ("ws_effects", "特效"),
    ("ws_audio", "音频"),
    ("ws_3d", "3D"),
    ("ws_export", "导出"),
    ("ws_home", "主页"),
    // Rune shell labels (Phase E)
    ("settings", "设置"),
    ("frame", "帧"),
    ("frames", "帧"),
    ("figures", "人物"),
    ("prev_frame", "上一帧"),
    ("next_frame", "下一帧"),
    ("fps", "帧率"),
    ("zoom", "缩放"),
    // Status line
    ("pose_mode", "姿态模式。"),
    (
        "status_ready",
        "拖动关节来摆姿势。手和脚使用 IK；拖动臀部来移动。",
    ),
    // UI mission additions (Wave 9 i18n audit)
    ("live_mcp_control", "Live MCP control"),
    ("live_mcp_off", "Live MCP off"),
    ("discard_changes", "Discard changes?"),
    ("rename", "Rename"),
    ("camera_delete_key", "Delete"),
    ("clear_key_no_undo", "Clear key (no undo)"),
    ("go_to_start", "Go to start"),
    ("go_to_end", "Go to end"),
    ("delete_frame_strokes", "Delete frame"),
    ("clip_missing_file", "missing file"),
    ("clip_unsupported_format", "unsupported format"),
    ("clip_unreadable_file", "unreadable file"),
    ("brush_preset", "Brush preset"),
    ("preset_size", "Preset size"),
    ("preset_opacity", "Preset opacity"),
    ("preset_scatter", "Preset scatter"),
    ("preset_nib", "Preset nib"),
    ("preset_smoothing", "Preset smoothing"),
    // Dopesheet keyframe navigation (Wave 10)
    ("next_keyframe", "Next keyframe"),
    ("previous_keyframe", "Previous keyframe"),
    ("no_keyframes_yet", "No keyframes yet"),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// For every language, every English key must have a real entry
    /// in that language's own table (not just an English fallback):
    /// non-empty and not the raw key.
    #[test]
    fn every_language_translates_every_key() {
        for lang in Lang::all() {
            for (key, _) in EN {
                let text = lookup(table_for(lang), key)
                    .unwrap_or_else(|| panic!("{key} missing in {:?}", lang));
                assert!(!text.is_empty(), "{key} is empty in {:?}", lang);
                assert_ne!(text, *key, "{key} in {:?} is just the key", lang);
                // And the public entry point agrees.
                assert_eq!(t(lang, key), text);
            }
        }
    }

    /// The Wave 9 panel keys must resolve in every language to a
    /// non-empty string that is not the raw key. English, Bengali
    /// and Spanish carry real translations; the other 25 languages
    /// currently carry the English text as their value.
    #[test]
    fn new_ui_keys_resolve_in_every_language() {
        const NEW_KEYS: [&str; 12] = [
            "help",
            "inspector",
            "easing",
            "colour_picker",
            "export_centre",
            "dopesheet",
            "hud_loop",
            "transport_first",
            "transport_last",
            "fx_panel",
            "clips_panel",
            "weather_panel",
        ];
        for lang in Lang::all() {
            for key in NEW_KEYS {
                let text = t(lang, key);
                assert!(!text.is_empty(), "{key} is empty in {:?}", lang);
                assert_ne!(text, key, "{key} in {:?} is just the key", lang);
                // Present in the language's own table, not only via
                // the English fallback in resolve().
                assert!(
                    lookup(table_for(lang), key).is_some(),
                    "{key} missing from {:?}'s own table",
                    lang
                );
            }
        }
        // Authored translations.
        assert_eq!(t(Lang::En, "help"), "Help");
        assert_eq!(t(Lang::Bn, "help"), "সাহায্য");
        assert_eq!(t(Lang::Es, "help"), "Ayuda");
        assert_eq!(t(Lang::Bn, "transport_last"), "শেষ ফ্রেম");
        assert_eq!(t(Lang::Es, "transport_first"), "Primer fotograma");
        assert_eq!(t(Lang::Es, "dopesheet"), "Hoja de tiempos");
        // Wave 10 authored translations.
        assert_eq!(t(Lang::Fr, "export_centre"), "Centre d'exportation");
        assert_eq!(t(Lang::ZhHans, "weather_panel"), "天气面板");
        // Wave 11 authored translations (all 28 languages now
        // carry real values for these keys).
        assert_eq!(t(Lang::Ar, "help"), "مساعدة");
        assert_eq!(t(Lang::Pl, "help"), "Pomoc");
        assert_eq!(t(Lang::Uk, "weather_panel"), "Панель погоди");
    }

    /// The UI-mission keys (Wave 9 completeness audit) must
    /// resolve in every language to a non-empty string that is
    /// not the raw key, present in each language's own table.
    /// Only English carries authored values so far; the other
    /// 27 languages carry the English text as a placeholder
    /// (the Wave 14 workspace-key convention), so every
    /// language currently resolves to the English string.
    #[test]
    fn mission_keys_resolve_in_every_language() {
        const MISSION_KEYS: [(&str, &str); 18] = [
            ("live_mcp_control", "Live MCP control"),
            ("live_mcp_off", "Live MCP off"),
            ("discard_changes", "Discard changes?"),
            ("rename", "Rename"),
            ("camera_delete_key", "Delete"),
            ("clear_key_no_undo", "Clear key (no undo)"),
            ("go_to_start", "Go to start"),
            ("go_to_end", "Go to end"),
            ("delete_frame_strokes", "Delete frame"),
            ("clip_missing_file", "missing file"),
            ("clip_unsupported_format", "unsupported format"),
            ("clip_unreadable_file", "unreadable file"),
            ("brush_preset", "Brush preset"),
            ("preset_size", "Preset size"),
            ("preset_opacity", "Preset opacity"),
            ("preset_scatter", "Preset scatter"),
            ("preset_nib", "Preset nib"),
            ("preset_smoothing", "Preset smoothing"),
        ];
        for lang in Lang::all() {
            for (key, english) in MISSION_KEYS {
                let text = t(lang, key);
                assert!(!text.is_empty(), "{key} is empty in {lang:?}");
                assert_ne!(text, key, "{key} in {lang:?} is just the key");
                assert!(
                    lookup(table_for(lang), key).is_some(),
                    "{key} missing from {lang:?}'s own table"
                );
                // Placeholder convention: untranslated languages
                // resolve to the English source string.
                assert_eq!(text, english, "{key} in {lang:?}");
            }
        }
    }

    /// The Wave 10 dopesheet keys must resolve in every
    /// language to a non-empty string that is not the raw
    /// key, present in each language's own table. Only
    /// English carries authored values so far; the other 27
    /// languages carry the English text as a placeholder
    /// (the UI-mission convention), so every language
    /// currently resolves to the English string. The
    /// panel's title key, `dopesheet`, predates the rebuild
    /// and is genuinely translated in all 28 languages.
    #[test]
    fn dopesheet_keys_resolve_in_every_language() {
        const DOPESHEET_KEYS: [(&str, &str); 3] = [
            ("next_keyframe", "Next keyframe"),
            ("previous_keyframe", "Previous keyframe"),
            ("no_keyframes_yet", "No keyframes yet"),
        ];
        for lang in Lang::all() {
            for (key, english) in DOPESHEET_KEYS {
                let text = t(lang, key);
                assert!(!text.is_empty(), "{key} is empty in {lang:?}");
                assert_ne!(text, key, "{key} in {lang:?} is just the key");
                assert!(
                    lookup(table_for(lang), key).is_some(),
                    "{key} missing from {lang:?}'s own table"
                );
                // Placeholder convention: untranslated languages
                // resolve to the English source string.
                assert_eq!(text, english, "{key} in {lang:?}");
            }
            // The title key is present in the language's own
            // table and never resolves to the raw key.
            let title = t(lang, "dopesheet");
            assert!(!title.is_empty(), "dopesheet is empty in {lang:?}");
            assert_ne!(title, "dopesheet", "dopesheet in {lang:?} is just the key");
            assert!(
                lookup(table_for(lang), "dopesheet").is_some(),
                "dopesheet missing from {lang:?}'s own table"
            );
        }
        assert_eq!(t(Lang::En, "dopesheet"), "Dopesheet");
    }

    #[test]
    fn all_tables_have_the_same_keys_without_duplicates() {
        for lang in Lang::all() {
            let table = table_for(lang);
            assert_eq!(table.len(), EN.len(), "size mismatch in {:?}", lang);
            let mut keys: Vec<&str> = table.iter().map(|(k, _)| *k).collect();
            let total = keys.len();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), total, "duplicate keys in {:?}", lang);
            for (key, _) in EN {
                assert!(
                    lookup(table, key).is_some(),
                    "{key} missing from {:?}",
                    lang
                );
            }
        }
    }

    #[test]
    fn shell_keys_resolve_in_every_language() {
        // The Phase E Rune-shell keys: present in every language's
        // own table (not only via the English fallback), non-empty,
        // never the raw key.
        const SHELL_KEYS: [&str; 8] = [
            "settings",
            "frame",
            "frames",
            "figures",
            "prev_frame",
            "next_frame",
            "fps",
            "zoom",
        ];
        for lang in Lang::all() {
            for key in SHELL_KEYS {
                let text = t(lang, key);
                assert!(!text.is_empty(), "{key} is empty in {:?}", lang);
                assert_ne!(text, key, "{key} in {:?} is just the key", lang);
                assert!(
                    lookup(table_for(lang), key).is_some(),
                    "{key} missing from {:?}'s own table",
                    lang
                );
            }
        }
        // Authored spot checks across scripts and language groups.
        assert_eq!(t(Lang::En, "settings"), "Settings");
        assert_eq!(t(Lang::De, "settings"), "Einstellungen");
        assert_eq!(t(Lang::Fr, "settings"), "Paramètres");
        assert_eq!(t(Lang::Bn, "prev_frame"), "আগের ফ্রেম");
        assert_eq!(t(Lang::Bn, "figures"), "ফিগারসমূহ");
        assert_eq!(t(Lang::Es, "next_frame"), "Fotograma siguiente");
        assert_eq!(t(Lang::Ru, "zoom"), "Масштаб");
        assert_eq!(t(Lang::Uk, "prev_frame"), "Попередній кадр");
        assert_eq!(t(Lang::ZhHans, "prev_frame"), "上一帧");
        assert_eq!(t(Lang::ZhHans, "fps"), "帧率");
        assert_eq!(t(Lang::Ar, "next_frame"), "الإطار التالي");
        assert_eq!(t(Lang::Tr, "frames"), "Kareler");
    }

    #[test]
    fn unknown_key_falls_back_to_the_key_itself() {
        for lang in Lang::all() {
            assert_eq!(t(lang, "no_such_key_xyz"), "no_such_key_xyz");
            assert_eq!(resolve(lang, "no_such_key_xyz"), None);
        }
    }

    #[test]
    fn missing_translation_falls_back_to_english() {
        // A table that only knows one key: every other key must
        // resolve through the English fallback, not to the key.
        static PARTIAL: &[(&str, &str)] = &[("play", "খেলুন")];
        assert_eq!(lookup(PARTIAL, "play"), Some("খেলুন"));
        let via_fallback = lookup(PARTIAL, "undo").or_else(|| lookup(EN, "undo"));
        assert_eq!(via_fallback, Some("Undo"));
        // English itself never falls back anywhere else.
        assert_eq!(resolve(Lang::En, "play"), Some("Play"));
    }

    #[test]
    fn codes_and_labels_for_all_languages() {
        // The original's 27 packs plus the port's Bengali.
        assert_eq!(Lang::all().len(), 28);
        let mut codes: Vec<&str> = Lang::all().iter().map(|l| l.code()).collect();
        let total = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), total, "duplicate language codes");
        for lang in Lang::all() {
            assert!(!lang.code().is_empty(), "empty code for {:?}", lang);
            assert!(!lang.label().is_empty(), "empty label for {:?}", lang);
        }
        // Exact codes, including the regional ones from the packs.
        assert_eq!(Lang::En.code(), "en");
        assert_eq!(Lang::Bn.code(), "bn");
        assert_eq!(Lang::Es.code(), "es");
        assert_eq!(Lang::Es419.code(), "es-419");
        assert_eq!(Lang::PtBr.code(), "pt-BR");
        assert_eq!(Lang::Nb.code(), "nb");
        assert_eq!(Lang::ZhHans.code(), "zh-Hans");
        assert_eq!(Lang::En.label(), "English");
        assert_eq!(Lang::Bn.label(), "বাংলা");
        assert_eq!(Lang::Es.label(), "Español");
        assert_eq!(Lang::PtBr.label(), "Português (Brasil)");
        assert_eq!(Lang::ZhHans.label(), "简体中文");
    }

    #[test]
    fn spot_checks() {
        assert_eq!(t(Lang::En, "play"), "Play");
        assert_eq!(t(Lang::Bn, "play"), "চালান");
        assert_eq!(t(Lang::Es, "play"), "Reproducir");
        assert_eq!(t(Lang::Bn, "figures_layers"), "ফিগারসমূহ (স্তর)");
        assert_eq!(t(Lang::Es, "figures_layers"), "FIGURAS (capas)");
        assert_eq!(t(Lang::Es, "onion_skin"), "Piel de cebolla");
        assert_eq!(t(Lang::Fr, "onion_skin"), "Pelure d'oignon");
        assert_eq!(t(Lang::PtBr, "onion_skin"), "Casca de cebola");
        assert_eq!(t(Lang::ZhHans, "onion_skin"), "洋葱皮");
        assert_eq!(t(Lang::De, "save"), "Speichern");
        assert_eq!(t(Lang::Ru, "save"), "Сохранить");
        assert_eq!(t(Lang::Ar, "save"), "حفظ");
        assert_eq!(t(Lang::De, "undo"), "Rückgängig");
        assert_eq!(t(Lang::Tr, "undo"), "Geri al");
        // Wave 14 workspace names.
        assert_eq!(t(Lang::Ru, "ws_graph"), "График");
        assert_eq!(t(Lang::ZhHans, "ws_effects"), "特效");
    }
}
