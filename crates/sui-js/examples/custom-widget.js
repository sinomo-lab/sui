"use strict";

const sui = require("..");

const app = new sui.App();
const marker = app.rgbaImage(
  2,
  1,
  Buffer.from([255, 255, 255, 255, 80, 180, 255, 255])
);

const meter = {
  name: "CPU meter",
  value: 0.62,
  marker,
  events: [],
  measure(constraints) {
    return constraints.clamp(new sui.Size(160, 28));
  },
  event(event) {
    this.events.push([event.kind, event.action]);
    return true;
  },
  semantics(semantics) {
    semantics.node(
      "progressBar",
      "CPU meter",
      this.value,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      0,
      1
    );
  },
  paint(paint) {
    const bounds = paint.bounds;
    const shadow = new sui.Shadow(0, 3, 8, 0, new sui.Color(0, 0, 0, 0.35));
    const clip = sui.Path.roundedRect(bounds, 6);
    paint.drawShadow(bounds, shadow, 6);
    paint.fillRoundedRect(bounds, new sui.Color(0.11, 0.12, 0.14, 1), 6);
    paint.pushClipPath(clip);
    paint.fillRect(
      new sui.Rect(bounds.x, bounds.y, bounds.width * this.value, bounds.height),
      new sui.Color(0.25, 0.68, 0.46, 1)
    );
    paint.popClip();
    paint.pushTransform(sui.Transform.translation(bounds.x + bounds.width - 24, bounds.y + 6));
    paint.drawImageQuad(
      [
        new sui.Point(0, 0),
        new sui.Point(18, 0),
        new sui.Point(18, 10),
        new sui.Point(0, 10),
      ],
      this.marker
    );
    paint.popTransform();
  },
};

const root = sui.Column(
  [
    new sui.Widget(meter),
    sui.Label("Host-driven custom widget"),
  ],
  8
);

const window = new sui.Window("Custom widget");
window.root(root);

app.window(window);

const running = app.start();
const snapshot = running.render();
console.log("commands:", snapshot.commandCount);
console.log("images:", snapshot.drawImageCount, snapshot.registeredImageCount);
running.handleEvent(
  sui.Event.pointer("down", new sui.Point(12, 12), undefined, undefined, "primary", 1)
);
console.log("events:", meter.events);
