import sinomo_ui as sui


class Meter:
    name = "Level meter"

    def __init__(self, value, marker):
        self.value = value
        self.marker = marker

    def measure(self, constraints):
        return constraints.clamp(sui.Size(160, 28))

    def event_with_context(self, event, context):
        if event.kind == "pointer" and event.action == "down" and event.button == "primary":
            bounds = context.bounds
            self.value = max(0.0, min(1.0, (event.position.x - bounds.x) / bounds.width))
            context.request_semantics()
            return True  # Mark the event handled and repaint.
        return False

    def semantics(self, semantics):
        semantics.node(
            role="progress_bar",
            name=self.name,
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
    sui.Window("Custom widget", size=sui.Size(320, 160)).root(
        sui.surface(
            sui.column(
                [sui.Widget(meter), sui.label("Click the meter to change its level.")],
                gap=16,
            ),
            role="window",
            padding=16,
            fill_width=True,
            fill_height=True,
        )
    )
)

if __name__ == "__main__":
    app.run()
