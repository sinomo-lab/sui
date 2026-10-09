// Open a desktop window. Node's event loop keeps running while it is open,
// so timers and async callbacks work as usual.
import * as sui from "@sui/ui";

const seconds = new sui.State(0);
const status = new sui.State("Ready");

async function load() {
  status.set("Loading…");
  await new Promise((resolve) => setTimeout(resolve, 1000));
  status.set("Loaded");
}

const app = new sui.App().window(
  new sui.Window("Hello from Node")
    .setInitialSize(new sui.Size(360, 220))
    .root(
      sui.column(
        [
          sui.label(seconds),
          sui.label(status),
          sui.button("Load", { onPress: load }),
        ],
        { gap: 8 },
      ),
    ),
);

const timer = setInterval(() => seconds.set(Number(seconds.get()) + 1), 1000);
try {
  await app.run();
} finally {
  clearInterval(timer);
}
