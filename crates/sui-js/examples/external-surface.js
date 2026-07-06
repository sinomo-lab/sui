"use strict";

const sui = require("..");

const pixels = Buffer.from([
  255, 60, 40, 255,
  40, 120, 255, 255,
  30, 180, 110, 255,
  255, 230, 80, 255,
]);
const texture = sui.ExternalTextureDescriptor.cpuRgba8(new sui.Size(2, 2), pixels, "1");
const surface = sui.ExternalSurface(
  texture,
  new sui.Size(128, 96),
  "CPU upload preview"
);

const root = sui.Column(
  [
    surface,
    sui.Label("CPU fallback external surface"),
  ],
  8
);

const window = new sui.Window("External surface");
window.root(root);

const app = new sui.App();
app.window(window);

const snapshot = app.start().render();
console.log("commands:", snapshot.commandCount);
console.log("images:", snapshot.drawImageCount, snapshot.registeredImageCount);
