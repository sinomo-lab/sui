// A browser app. Serve the package directory (crates/sui-js) over HTTP after
// `cargo xtask web build`, then open examples/web/index.html.
import * as sui from "../../web.js";

const count = new sui.State(0);
const status = new sui.State("Ready");

async function load() {
  status.set("Loading…");
  await new Promise((resolve) => setTimeout(resolve, 1000));
  status.set(`Loaded at ${new Date().toLocaleTimeString()}`);
}

const app = new sui.App().setTheme(sui.Theme.dark()).window(
  new sui.Window("SUI in the browser").root(
    sui.padding(
      sui.column(
        [
          sui.label("Hello from WebAssembly", { fontSize: 20 }),
          sui.label(count),
          sui.button("Increment", { onPress: () => count.set(Number(count.get()) + 1) }),
          sui.button("Load", { onPress: load }),
          sui.label(status),
        ],
        { gap: 8 },
      ),
      { padding: 16 },
    ),
  ),
);

app.on("ping", (payload) => console.log("ping", payload));

await app.run({
  canvas: "app",
  onReady: (ui) => ui.emit("ping", "ready"),
});
console.log("app exited");
