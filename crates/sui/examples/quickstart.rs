use sui::prelude::*;

fn main() -> Result<()> {
    let theme = DefaultTheme::light();

    let content = Stack::vertical()
        .spacing(12.0)
        .alignment(Alignment::Start)
        .with_child(
            Label::new("Your first SUI window")
                .theme(theme)
                .font_size(24.0)
                .line_height(30.0)
                .color(theme.palette.text),
        )
        .with_child(
            Label::new("Widgets are retained Rust values arranged into one tree.")
                .theme(theme)
                .color(theme.palette.text_muted),
        )
        .with_child(
            Button::new("Say hello")
                .theme(theme)
                .on_press(|| println!("Hello from SUI!")),
        );

    let root = Surface::window(content)
        .theme(theme)
        .padding(Insets::all(24.0))
        .fill();

    App::new()
        .window(Window::new("SUI Quickstart").root(root))
        .run()
}
