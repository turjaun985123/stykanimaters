use gpui::{App, AppContext, Application, WindowOptions};
use stykanimate_rs::app;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|cx| app::EditorView::new(cx))
        })
        .expect("failed to open window");
    });
}
