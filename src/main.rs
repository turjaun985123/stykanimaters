mod app;
mod audio;
mod editor;
mod export;
mod i18n;
mod model;
mod nodes;
mod pivot;
mod projects;
mod svg;
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
