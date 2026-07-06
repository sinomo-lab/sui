import sui


class Meter:
    name = "CPU meter"

    def __init__(self, value, marker):
        self.value = value
        self.marker = marker
        self.events = []

    def measure(self, constraints):
        return constraints.clamp(sui.Size(160, 28))

    def event(self, event):
        self.events.append((event.kind, event.action))
        return True

    def semantics(self, semantics):
        semantics.node(
            role="progress_bar",
            name="CPU meter",
            value=self.value,
            min_value=0.0,
            max_value=1.0,
        )

    def paint(self, paint):
        bounds = paint.bounds
        shadow = sui.Shadow(0, 3, 8, 0, sui.Color.rgba(0, 0, 0, 0.35))
        clip = sui.Path.rounded_rect(bounds, 6)
        paint.draw_shadow(bounds, shadow, radii=6)
        paint.fill_rounded_rect(bounds, sui.Color.rgba(0.11, 0.12, 0.14, 1.0), radii=6)
        paint.push_clip_path(clip)
        paint.fill_rect(
            sui.Rect(bounds.x, bounds.y, bounds.width * self.value, bounds.height),
            sui.Color.rgba(0.25, 0.68, 0.46, 1.0),
        )
        paint.pop_clip()
        paint.push_transform(sui.Transform.translation(bounds.x + bounds.width - 24, bounds.y + 6))
        paint.draw_image_quad(
            [
                sui.Point(0, 0),
                sui.Point(18, 0),
                sui.Point(18, 10),
                sui.Point(0, 10),
            ],
            self.marker,
        )
        paint.pop_transform()


app = sui.App()
marker = app.rgba_image(2, 1, bytes([255, 255, 255, 255, 80, 180, 255, 255]))
meter = Meter(0.62, marker)
app.window(
    sui.Window("Custom widget").root(
        sui.Column(
            [
                sui.Widget(meter),
                sui.Label("Host-driven custom widget"),
            ],
            gap=8,
        )
    )
)

running = app.start()
snapshot = running.render()
print("commands:", snapshot.command_count)
print("images:", snapshot.draw_image_count, snapshot.registered_image_count)
running.handle_event(
    sui.Event.pointer("down", sui.Point(12, 12), button="primary", buttons=1)
)
print("events:", meter.events)
