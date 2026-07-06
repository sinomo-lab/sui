import sui


count = sui.State(0)
enabled = sui.State(True)
opacity = sui.State(0.65)
name = sui.State("")


def increment():
    count.set(count.get() + 1)


app = sui.App()
root = sui.Column(
    [
        sui.Label(count),
        sui.Button("Increment", on_press=increment),
        sui.Checkbox("Enabled", enabled),
        sui.Switch("Preview", True),
        sui.Slider("Opacity", opacity, min_value=0.0, max_value=1.0, step=0.05),
        sui.TextInput("Name", name, placeholder="Optional label"),
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
