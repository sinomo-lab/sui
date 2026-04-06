use sui::prelude::*;
use sui_widget_book::{
    LivePerformanceRoot, build_button_grid_benchmark, build_widget_book_gallery,
    default_widget_book_state, register_widget_book_images,
};

const WINDOW_TITLE: &str = "SUI Dev";
const WINDOW_DESCRIPTION: &str =
    "Tabbed development host for the widget book and focused performance demos.";
const DEV_TABS_NAME: &str = "SUI Dev tabs";
const WIDGET_BOOK_TAB_LABEL: &str = "Widget book";
const BUTTON_GRID_TAB_LABEL: &str = "64 buttons";

fn build_dev_application() -> Application {
    let widget_book_state = default_widget_book_state();

    let mut app = Application::new();
    register_widget_book_images(&mut app);
    app.window(
        WindowBuilder::new().title(WINDOW_TITLE).root(LivePerformanceRoot::new(
            WINDOW_TITLE,
            WINDOW_DESCRIPTION,
            Tabs::new(DEV_TABS_NAME)
                .selected(0)
                .tab(
                    WIDGET_BOOK_TAB_LABEL,
                    build_widget_book_gallery(widget_book_state),
                )
                .tab(BUTTON_GRID_TAB_LABEL, build_button_grid_benchmark()),
        )),
    )
}

fn main() -> sui::Result<()> {
    build_dev_application().run()
}
