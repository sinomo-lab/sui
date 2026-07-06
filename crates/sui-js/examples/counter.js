"use strict";

const sui = require("..");

const count = new sui.State(0);
const enabled = new sui.State(true);
const opacity = new sui.State(0.65);
const name = new sui.State("");

function increment() {
  count.set(Number(count.get()) + 1);
}

const root = sui.Column(
  [
    sui.Label(count),
    sui.Button("Increment", increment),
    sui.Checkbox("Enabled", enabled),
    sui.Switch("Preview", true),
    sui.Slider("Opacity", opacity, 0, 1, 0.05),
    sui.TextInput("Name", name, "Optional label"),
  ],
  8
);

const window = new sui.Window("Counter");
window.root(root);

const app = new sui.App();
app.window(window);

const running = app.start();
console.log("initial commands:", running.render().commandCount);

running.uiHandle().post(increment);
running.drain();
console.log("count:", count.get());
console.log("updated commands:", running.render().commandCount);
