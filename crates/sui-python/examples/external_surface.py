import sui


pixels = bytes(
    [
        255,
        60,
        40,
        255,
        40,
        120,
        255,
        255,
        30,
        180,
        110,
        255,
        255,
        230,
        80,
        255,
    ]
)
texture = sui.ExternalTextureDescriptor.cpu_rgba8(sui.Size(2, 2), pixels, generation=1)
surface = sui.ExternalSurface(
    texture,
    desired_size=sui.Size(128, 96),
    name="CPU upload preview",
)

app = sui.App()
app.window(
    sui.Window("External surface").root(
        sui.Column(
            [
                surface,
                sui.Label("CPU fallback external surface"),
            ],
            gap=8,
        )
    )
)

snapshot = app.start().render()
print("commands:", snapshot.command_count)
print("images:", snapshot.draw_image_count, snapshot.registered_image_count)
