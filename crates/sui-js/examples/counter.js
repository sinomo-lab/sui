import * as sui from "@sui/ui";

const count = new sui.State(0);
const enabled = new sui.State(true);
const opacity = new sui.State(0.65);
const name = new sui.State("");

function increment() {
  count.set(Number(count.get()) + 1);
}

// Callbacks may be async; the app keeps running while they wait.
async function incrementLater() {
  await new Promise((resolve) => setTimeout(resolve, 10));
  increment();
}

const theme = sui.Theme.dark();
const root = sui.column(
  [
    sui.label(count),
    sui.button("Increment", { onPress: increment }),
    sui.button("Increment later", { onPress: incrementLater }),
    sui.checkbox("Enabled", { checked: enabled }),
    sui.switchControl("Preview", { checked: true }),
    sui.slider("Opacity", { value: opacity, min: 0, max: 1, step: 0.05 }),
    sui.textInput("Name", { value: name, placeholder: "Optional label" }),
  ],
  { gap: 8 },
);

const app = new sui.App()
  .setTheme(theme)
  .window(new sui.Window("Counter").root(root));

// `app.start()` runs the app without a platform; `await app.run()` would
// open the window instead.
const running = app.start();
console.log("initial commands:", running.render().commandCount);

running.click(running.render().getOne({ role: "button", name: "Increment later" }));
await running.idle();
console.log("count:", count.get());
console.log("updated commands:", running.render().commandCount);

theme.setPreset("light");
running.drain();
console.log("theme updated commands:", running.render().commandCount);
