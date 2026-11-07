mod app;
mod editor;
mod export;
mod model;

use gpui::{App, AppContext, Application, WindowOptions};

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| app::EditorView::new())
        })
        .expect("failed to open window");
    });
}
