mod app;
mod dialogs;
mod audio;
mod audio_edit;
mod editor;
mod export;
mod fig3d;
mod graph;
mod graph_panel;
mod i18n;
mod layers;
mod media;
mod model;
mod nodes;
mod nodes_anim;
mod paint;
mod pivot;
mod projects;
mod shell;
mod style;
mod svg;
mod text_edit;
mod touch;
mod vfx;

use gpui::{App, AppContext, Application, WindowOptions};

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|cx| app::EditorView::new(cx))
        })
        .expect("failed to open window");
    });
}
