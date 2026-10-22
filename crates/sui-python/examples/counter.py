import sinomo_ui as sui


count = sui.State(0)
enabled = sui.State(True)


def increment():
    count.set(count.get() + 1)


theme = sui.Theme.dark()
app = sui.App(theme=theme)
root = sui.column(
    [
        sui.label(count),
        sui.button("Increment", on_press=increment, enabled=enabled),
        sui.checkbox("Enabled", enabled),
        sui.switch(
            "Dark theme", True,
            on_change=lambda dark: theme.set_preset("dark" if dark else "light"),
        ),
    ],
    gap=8,
)

app.window(
    sui.Window("Counter", size=sui.Size(320, 220)).root(
        sui.surface(root, role="window", padding=16, fill_width=True, fill_height=True)
    )
)

if __name__ == "__main__":
    app.run()
