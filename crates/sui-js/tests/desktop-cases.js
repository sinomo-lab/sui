// Real-window cases for desktop.test.js. winit allows one event loop per
// process, so each case runs in its own process:
//   node tests/desktop-cases.js <case>
// A case prints its result as JSON and exits 0, or exits 1 on failure.
import { sui } from "./support.js";

function makeApp() {
  return new sui.App().window(new sui.Window("Desktop test").root(sui.label("Desktop test")));
}

const cases = {
  async "request-exit"() {
    await makeApp().run({ onReady: (ui) => setTimeout(() => ui.requestExit(), 300) });
    return "returned";
  },

  async "abort-signal"() {
    await makeApp().run({ signal: AbortSignal.timeout(300) });
    return "returned";
  },

  async "throwing-on-ready"() {
    try {
      await makeApp().run({
        onReady() {
          throw new Error("setup failed");
        },
      });
    } catch (error) {
      return error.message;
    }
    return "resolved";
  },

  async timers() {
    let ticks = 0;
    let interval;
    await makeApp().run({
      onReady(ui) {
        // Count from the first event-loop turn on, which creates the window
        // and its renderer and may block for a while.
        setTimeout(() => {
          interval = setInterval(() => ticks++, 20);
          setTimeout(() => ui.requestExit(), 1000);
        }, 0);
      },
    });
    clearInterval(interval);
    return ticks;
  },

  async "run-twice"() {
    const first = makeApp();
    let secondError = null;
    await first.run({
      async onReady(ui) {
        try {
          await makeApp().run();
        } catch (error) {
          secondError = error.message;
        }
        ui.requestExit();
      },
    });
    return secondError;
  },
};

const name = process.argv[2];
const run = cases[name];
if (run === undefined) {
  console.error(`unknown desktop case: ${name}`);
  process.exit(2);
}
try {
  console.log(JSON.stringify(await run()));
} catch (error) {
  console.error(error);
  process.exit(1);
}
