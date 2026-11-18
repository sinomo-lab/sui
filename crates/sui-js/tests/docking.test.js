// Docking: tab options, panels added and removed at runtime, and close requests.
import assert from "node:assert/strict";
import { test } from "node:test";
import { capturingErrors, sui } from "./support.js";

function start(root) {
  const window = new sui.Window("Docking").setInitialSize(new sui.Size(900, 600)).root(root);
  return new sui.App().window(window).start();
}

function tabsState(...ids) {
  return new sui.DockState(new sui.DockLayout(sui.DockNode.tabs(ids)));
}

test("panel options describe a tab", () => {
  const options = new sui.DockPanelOptions("main.rs", "folder", true, true, new sui.Size(240, 120));
  assert.equal(options.title, "main.rs");
  assert.equal(options.icon, "folder");
  assert.equal(options.closable, true);
  assert.equal(options.dirty, true);
  assert.equal(options.minSize.width, 240);

  const plain = new sui.DockPanelOptions("Output");
  assert.equal(plain.icon, null);
  assert.equal(plain.closable, false);
  assert.equal(plain.minSize.height, 0);

  const spec = new sui.DockPanelSpec("1", "Files", sui.label("Files"), undefined, true);
  assert.equal(spec.title, "Files");
  assert.equal(spec.options.closable, true);

  assert.throws(() => new sui.DockPanelOptions("Bad", "not-an-icon"));
  assert.throws(() => new sui.DockPanelOptions("Bad", undefined, false, false, new sui.Size(-1, 0)));
  assert.throws(() => sui.DockPlacement.root("diagonal"));
});

test("panels are added, updated, reordered, and removed while mounted", () => {
  const state = tabsState("1");
  const built = [];
  const running = start(
    sui.dockWorkspace(state, [new sui.DockPanelSpec("1", "Files", sui.label("Files panel"))], {
      panelBuilder(panel) {
        built.push(panel);
        return sui.label(`Built panel ${panel}`);
      },
    }),
  );
  running.render();

  assert.throws(() => state.addPanel("1", new sui.DockPanelOptions("Files")));
  state.addPanel("2", new sui.DockPanelOptions("Search"), sui.DockPlacement.beside("1", "right"));
  state.addPanel("3", new sui.DockPanelOptions("Output"), sui.DockPlacement.hidden());
  state.addPanel("4", new sui.DockPanelOptions("Notes"));
  assert.equal(state.isRegistered("2"), true);
  assert.deepEqual(state.snapshot().hidden, ["3"]);

  let snapshot = running.render();
  assert.deepEqual(built, ["2", "3", "4"]);
  assert.equal(snapshot.find({ name: "Built panel 2" }).length, 1);

  // Panel 4 joined panel 1's group; move it in front.
  assert.equal(state.dockTab("4", "1", false), true);
  state.updatePanel("4", new sui.DockPanelOptions("Notes (edited)", undefined, false, true));
  snapshot = running.render();
  const tab = snapshot.getOne({ role: "button", name: "Notes (edited)" });
  assert.ok(tab.bounds.x < snapshot.getOne({ role: "button", name: "Files" }).bounds.x);

  assert.equal(state.removePanel("4"), true);
  assert.equal(state.removePanel("4"), false);
  assert.equal(state.isRegistered("4"), false);
  assert.throws(() => state.updatePanel("4", new sui.DockPanelOptions("Gone")));
  assert.equal(running.render().find({ name: "Notes (edited)" }).length, 0);
});

test("close requests hide, keep, or remove a panel", () => {
  const state = tabsState("1", "2", "3");
  const actions = { 1: "keep", 2: "remove", 3: undefined };
  const asked = [];
  const panel = (id, title) =>
    new sui.DockPanelSpec(id, title, sui.label(`${title} panel`), undefined, true);
  const running = start(
    sui.dockWorkspace(state, [panel("1", "One"), panel("2", "Two"), panel("3", "Three")], {
      onCloseRequest(id) {
        asked.push(id);
        return actions[id];
      },
    }),
  );
  const close = (title) =>
    running.click(running.render().getOne({ role: "button", name: `Close ${title}` }));

  close("One");
  assert.equal(state.snapshot().hidden.length, 0);
  close("Two");
  assert.equal(state.isRegistered("2"), false);
  close("Three");
  assert.deepEqual(state.snapshot().hidden, ["3"]);
  assert.equal(state.isRegistered("3"), true);
  assert.deepEqual(asked, ["1", "2", "3"]);
});

test("a failing close handler keeps the panel open", async () => {
  const state = tabsState("1");
  const running = start(
    sui.dockWorkspace(
      state,
      [new sui.DockPanelSpec("1", "One", sui.label("One panel"), undefined, true)],
      {
        onCloseRequest() {
          throw new RangeError("close failed");
        },
      },
    ),
  );
  const errors = await capturingErrors(() => {
    running.click(running.render().getOne({ role: "button", name: "Close One" }));
  });
  assert.equal(errors.length, 1);
  assert.deepEqual(state.snapshot().hidden, []);
  assert.equal(state.isRegistered("1"), true);
});

test("retainPanels prunes a saved layout", () => {
  const saved = new sui.DockLayout(
    sui.DockNode.split("horizontal", 0.3, sui.DockNode.tabs(["1"]), sui.DockNode.tabs(["9"])),
    [new sui.DockFloatingGroup("1", ["8"], "8", new sui.Rect(10, 10, 300, 200))],
    ["7"],
  );
  const pruned = saved.retainPanels(["1"]);
  assert.deepEqual(pruned.floating, []);
  assert.deepEqual(pruned.hidden, []);
  // The remaining layout is valid and holds only panel 1.
  const state = new sui.DockState(pruned);
  assert.equal(state.hide("1"), true);
  assert.throws(() => state.activate("9"));
});
