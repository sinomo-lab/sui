use sui::prelude::*;

fn main() -> Result<()> {
    App::new()
        .main_window("Hello SUI", Label::new("Ready"))
        .run()
}
