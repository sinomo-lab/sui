"use strict";

const sui = require("..");

const count = new sui.State(0);
const enabled = new sui.State(true);
const opacity = new sui.State(0.65);
const name = new sui.State("");

function increment() {
  count.set(Number(count.get()) + 1);
}

const theme = sui.Theme.dark();
const root = sui.column(
  [
    sui.label(count),
    sui.button("Increment", { onPress: increment }),
    sui.checkbox("Enabled", { checked: enabled }),
    sui.switchControl("Preview", { on: true }),
    sui.slider("Opacity", { value: opacity, min: 0, max: 1, step: 0.05 }),
    sui.textInput("Name", { value: name, placeholder: "Optional label" }),
  ],
  { gap: 8 }
);

const window = new sui.Window("Counter");
window.root(root);

const app = new sui.App();
app.setTheme(theme);
app.window(window);

const running = app.start();
console.log("initial commands:", running.render().commandCount);

running.uiHandle().post(increment);
running.drain();
console.log("count:", count.get());
console.log("updated commands:", running.render().commandCount);

theme.setPreset("light");
running.drain();
console.log("theme updated commands:", running.render().commandCount);
