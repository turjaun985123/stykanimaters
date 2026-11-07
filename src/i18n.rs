//! Localisation string tables for the editor UI.
//!
//! Keys are stable identifiers named after the control or message
//! they label (see `src/app.rs` for the controls and `src/editor.rs`
//! for the status strings). `t` resolves a key in the requested
//! language, falls back to English when a translation is missing,
//! and finally falls back to the key itself so an untranslated UI
//! still shows something debuggable instead of a blank.
//!
//! Not yet wired into the view: registering this module and
//! swapping the literals in `app.rs` for `t(...)` calls is a
//! separate integration step.

/// Languages the UI can be displayed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Bn,
    Es,
}

impl Lang {
    /// ISO 639-1 code, e.g. for a settings file or `<html lang>`.
    pub fn code(&self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Bn => "bn",
            Lang::Es => "es",
        }
    }

    /// The language's name in itself, for a language picker.
    pub fn label(&self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Bn => "বাংলা",
            Lang::Es => "Español",
        }
    }

    /// All languages, in picker order.
    pub fn all() -> [Lang; 3] {
        [Lang::En, Lang::Bn, Lang::Es]
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
    // Status line
    ("pose_mode", "Pose mode."),
    (
        "status_ready",
        "Drag a joint to pose. Hands and feet use IK; drag the hips to move.",
    ),
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
    // Status line
    ("pose_mode", "পোজ মোড।"),
    (
        "status_ready",
        "পোজ দেওয়ার জন্য একটি জয়েন্ট টেনে ধরুন। হাত ও পা IK দিয়ে চলে; ফিগার সরাতে কোমর টানুন।",
    ),
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
    // Status line
    ("pose_mode", "Modo pose."),
    (
        "status_ready",
        "Arrastra una articulación para posar. Las manos y los pies usan IK; arrastra las caderas para mover.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_english_key_is_translated_in_bn_and_es() {
        for (key, _) in EN {
            for lang in [Lang::Bn, Lang::Es] {
                let text = lookup(table_for(lang), key)
                    .unwrap_or_else(|| panic!("{key} missing in {:?}", lang));
                assert!(!text.is_empty(), "{key} is empty in {:?}", lang);
                assert_ne!(text, *key, "{key} in {:?} is just the key", lang);
                // And the public entry point agrees.
                assert_eq!(t(lang, key), text);
            }
        }
    }

    #[test]
    fn tables_have_the_same_keys_without_duplicates() {
        assert_eq!(EN.len(), BN.len());
        assert_eq!(EN.len(), ES.len());
        for table in [EN, BN, ES] {
            let mut keys: Vec<&str> = table.iter().map(|(k, _)| *k).collect();
            let total = keys.len();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(keys.len(), total, "duplicate keys in a table");
            for (key, _) in EN {
                assert!(lookup(table, key).is_some(), "{key} missing from a table");
            }
        }
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
    fn codes_and_labels() {
        assert_eq!(Lang::En.code(), "en");
        assert_eq!(Lang::Bn.code(), "bn");
        assert_eq!(Lang::Es.code(), "es");
        assert_eq!(Lang::En.label(), "English");
        assert_eq!(Lang::Bn.label(), "বাংলা");
        assert_eq!(Lang::Es.label(), "Español");
    }

    #[test]
    fn spot_checks() {
        assert_eq!(t(Lang::En, "play"), "Play");
        assert_eq!(t(Lang::Bn, "play"), "চালান");
        assert_eq!(t(Lang::Es, "play"), "Reproducir");
        assert_eq!(t(Lang::Bn, "figures_layers"), "ফিগারসমূহ (স্তর)");
        assert_eq!(t(Lang::Es, "figures_layers"), "FIGURAS (capas)");
        assert_eq!(t(Lang::Es, "onion_skin"), "Piel de cebolla");
    }
}
