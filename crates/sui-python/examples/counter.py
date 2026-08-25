import sui


count = sui.State(0)
enabled = sui.State(True)
opacity = sui.State(0.65)
name = sui.State("")


def increment():
    count.set(count.get() + 1)


theme = sui.Theme.dark()
app = sui.App(theme=theme)
root = sui.column(
    [
        sui.label(count),
        sui.button("Increment", on_press=increment),
        sui.checkbox("Enabled", enabled),
        sui.switch("Preview", True),
        sui.slider("Opacity", opacity, min_value=0.0, max_value=1.0, step=0.05),
        sui.text_input("Name", name, placeholder="Optional label"),
    ],
    gap=8,
)
app.window(sui.Window("Counter").root(root))

running = app.start()
print("initial commands:", running.render().command_count)

running.ui_handle().post(increment)
running.drain()
print("count:", count.get())
print("updated commands:", running.render().command_count)

theme.set_preset("light")
running.drain()
print("theme updated commands:", running.render().command_count)
