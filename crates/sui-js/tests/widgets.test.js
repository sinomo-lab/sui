// Widget behavior: state bindings, overlays, layout, and controllers.
import assert from "node:assert/strict";
import { test } from "node:test";
import { capturingErrors, sui } from "./support.js";

function start(root) {
  // A sized window gets a viewport, as a platform host would provide.
  const window = new sui.Window("Widgets").setInitialSize(new sui.Size(640, 480)).root(root);
  return new sui.App().window(window).start();
}

function names(snapshot, role) {
  return snapshot.find({ role }).map((node) => node.name);
}

function bounds(snapshot, role, name) {
  return snapshot.getOne({ role, name }).bounds;
}

function approx(actual, expected, tolerance = 1e-6) {
  assert.ok(
    Math.abs(actual - expected) <= tolerance,
    `expected ${actual} to be within ${tolerance} of ${expected}`,
  );
}

test("state-bound labels update in place", () => {
  const label = new sui.State("Save");
  const running = start(
    sui.column([
      sui.button(label),
      sui.checkbox(label),
      sui.switchControl(label),
      sui.radioButton(label),
    ]),
  );
  assert.deepEqual(names(running.render(), "button"), ["Save"]);

  label.set("Saved");
  running.drain();
  const snapshot = running.render();

  assert.deepEqual(names(snapshot, "button"), ["Saved"]);
  assert.deepEqual(names(snapshot, "checkbox"), ["Saved"]);
});

test("dialog follows state and clears it on dismiss", () => {
  const shown = new sui.State(false);
  const dismissed = [];
  const running = start(
    sui.dialog("Confirm", sui.label("Apply the change?"), {
      open: shown,
      actions: [sui.button("Apply")],
      onDismiss: () => dismissed.push(true),
    }),
  );
  assert.equal(running.render().find({ role: "dialog" }).length, 0);

  shown.set(true);
  running.drain();
  const snapshot = running.render();
  assert.equal(snapshot.find({ role: "dialog" }).length, 1);
  assert.ok(names(snapshot, "button").includes("Apply"));

  running.press(snapshot.getOne({ role: "dialog" }), "Escape");
  running.drain();
  assert.equal(shown.get(), false);
  assert.deepEqual(dismissed, [true]);
  assert.equal(running.render().find({ role: "dialog" }).length, 0);
});

test("tabs show panels and report changes", () => {
  const selected = new sui.State(0);
  const changes = [];
  const running = start(
    sui.tabs("Sections", ["Design", "Inspect"], {
      selected,
      panels: [sui.label("Design body"), sui.label("Inspect body")],
      onChange: (index, label) => changes.push([index, label]),
    }),
  );
  assert.ok(running.render().find({ text: "Design body" }).length > 0);

  selected.set(1);
  running.drain();
  const snapshot = running.render();
  assert.ok(snapshot.find({ text: "Inspect body" }).length > 0);

  running.press(snapshot.getOne({ role: "tabs" }), "ArrowLeft");
  running.drain();
  assert.equal(selected.get(), 0);
  assert.deepEqual(changes, [[0, "Design"]]);
});

test("tabs reject mismatched panels", () => {
  assert.throws(
    () => sui.tabs("Bad", ["One", "Two"], { panels: [sui.label("Only one")] }),
    /one panel per tab/,
  );
});

test("popover follows state", () => {
  const shown = new sui.State(false);
  const running = start(
    sui.popover("Details", sui.button("More"), sui.label("Popover body"), { open: shown }),
  );
  assert.equal(running.render().find({ text: "Popover body" }).length, 0);

  shown.set(true);
  running.drain();
  assert.ok(running.render().find({ text: "Popover body" }).length > 0);
});

test("enabled disables controls and follows state", () => {
  const enabled = new sui.State(false);
  const pressed = [];
  const running = start(
    sui.column([
      sui.button("Save", { onPress: () => pressed.push(true), enabled }),
      sui.checkbox("Agree", { enabled: false }),
      sui.slider("Volume", { enabled: false }),
      sui.textInput("Name", { enabled: false }),
    ]),
  );
  let snapshot = running.render();
  assert.equal(snapshot.getOne({ role: "button", name: "Save" }).disabled, true);
  assert.equal(snapshot.getOne({ role: "checkbox" }).disabled, true);
  assert.equal(snapshot.getOne({ role: "slider" }).disabled, true);
  // Like a test locator, clicking refuses a control that cannot act.
  assert.throws(
    () => running.click(snapshot.getOne({ role: "button", name: "Save" })),
    /not actionable/,
  );
  assert.deepEqual(pressed, []);

  enabled.set(true);
  running.drain();
  snapshot = running.render();
  const save = snapshot.getOne({ role: "button", name: "Save" });
  assert.equal(save.disabled, false);
  running.click(save);
  assert.deepEqual(pressed, [true]);
});

test("spacer and flex items share the main axis", () => {
  const running = start(
    sui.column(
      [
        sui.row([sui.label("Title"), sui.spacer(), sui.button("Save")]),
        sui.row([sui.flexItem(sui.button("Grow"), { grow: 1 }), sui.button("Fixed")]),
      ],
      // A column sizes its cross axis to its content unless it stretches.
      { alignItems: "stretch" },
    ),
  );
  const snapshot = running.render();

  const save = bounds(snapshot, "button", "Save");
  assert.ok(save.x + save.width > 600);
  const grow = bounds(snapshot, "button", "Grow");
  const fixed = bounds(snapshot, "button", "Fixed");
  assert.ok(grow.width > 3 * fixed.width);
  assert.ok(fixed.x + fixed.width > 600);
});

test("column justify and alignment", () => {
  const running = start(
    sui.sizedBox({
      child: sui.column([sui.button("A"), sui.button("Longer button")], {
        justify: "end",
        alignItems: "center",
      }),
      width: 640,
      height: 480,
    }),
  );
  const snapshot = running.render();
  const short = bounds(snapshot, "button", "A");
  const long = bounds(snapshot, "button", "Longer button");

  assert.ok(long.y + long.height > 440);
  approx(short.x + short.width / 2, long.x + long.width / 2, 1);
});

test("flex item outside a flex container shows its child", () => {
  const running = start(sui.flexItem(sui.button("Alone"), { grow: 1 }));
  assert.ok(running.render().getOne({ role: "button", name: "Alone" }));
});

test("unknown justify is rejected", () => {
  assert.throws(() => sui.row([], { justify: "sideways" }), /unknown justify/);
});

test("button presentation and semantics options", () => {
  const running = start(
    sui.row([
      sui.button("Delete", {
        appearance: "filled",
        tone: "danger",
        icon: "close",
        minWidth: 160,
        semanticName: "Delete project",
        description: "Removes the project permanently",
      }),
      sui.button("Cancel", { appearance: "ghost" }),
    ]),
  );
  const snapshot = running.render();
  const remove = snapshot.getOne({ role: "button", name: "Delete project" });
  assert.equal(remove.description, "Removes the project permanently");
  assert.ok(remove.bounds.width >= 160);

  assert.throws(() => sui.button("Bad", { appearance: "glossy" }), /unknown button appearance/);
});

test("text field submit, focus, and read-only", () => {
  const submitted = [];
  const focus = [];
  const running = start(
    sui.column([
      sui.textInput("Search", {
        onSubmit: (value) => submitted.push(value),
        onFocusChange: (focused) => focus.push(focused),
      }),
      sui.textArea("Notes", { value: "Fixed", readOnly: true }),
    ]),
  );
  const search = running.render().getOne({ role: "text_input", name: "Search" });
  running.fill(search, "sui");
  running.press(running.render().getOne({ role: "text_input", name: "Search" }), "Enter");

  assert.deepEqual(submitted, ["sui"]);
  assert.equal(focus[0], true);
  assert.equal(running.render().getOne({ name: "Notes" }).editable, false);
});

test("semanticName overrides the visible text", () => {
  const running = start(
    sui.column([
      sui.checkbox("On", { semanticName: "Enable sync" }),
      sui.switchControl("On", { semanticName: "Dark mode" }),
      sui.radioButton("A", { semanticName: "Option A" }),
      sui.label("42%", { semanticName: "Battery level" }),
    ]),
  );
  const snapshot = running.render();
  assert.equal(snapshot.getOne({ role: "checkbox" }).name, "Enable sync");
  assert.equal(snapshot.getOne({ role: "switch" }).name, "Dark mode");
  assert.equal(snapshot.getOne({ role: "radio_button" }).name, "Option A");
  assert.ok(snapshot.find({ name: "Battery level" }).length > 0);
});

test("rebuildOnChange rebuilds the subtree", () => {
  const count = new sui.State(1);
  const title = new sui.State("Draft");
  const builds = [];

  const running = start(
    sui.rebuildOnChange([count], () => {
      const n = count.get();
      builds.push(n);
      return sui.column([
        sui.label(title),
        ...Array.from({ length: n }, (_, i) => sui.label(`Item ${i}`)),
      ]);
    }),
  );
  let snapshot = running.render();
  assert.ok(snapshot.find({ text: "Item 0" }).length > 0);
  assert.equal(snapshot.find({ text: "Item 1" }).length, 0);

  count.set(3);
  running.drain();
  snapshot = running.render();
  assert.ok(snapshot.find({ text: "Item 2" }).length > 0);
  assert.deepEqual(builds, [1, 3]);

  // States used inside the rebuilt subtree stay live.
  title.set("Final");
  running.drain();
  assert.ok(running.render().find({ text: "Final" }).length > 0);
  assert.deepEqual(builds, [1, 3]);
});

test("rebuildOnChange reports builder errors", async () => {
  const broken = new sui.State(false);
  const errors = await capturingErrors(() => {
    const running = start(
      sui.rebuildOnChange([broken], () => {
        if (broken.get()) {
          throw new Error("cannot build");
        }
        return sui.label("Fine");
      }),
    );
    assert.ok(running.render().find({ text: "Fine" }).length > 0);

    broken.set(true);
    running.drain();
    assert.equal(running.render().find({ text: "Fine" }).length, 0);
  });
  assert.deepEqual(
    errors.map((error) => error.message),
    ["cannot build"],
  );
});

test("scroll controller scrolls and reports metrics", () => {
  const controller = new sui.ScrollController();
  const rows = sui.column(
    Array.from({ length: 100 }, (_, i) => sui.label(`Row ${i}`)),
    { alignItems: "stretch" },
  );
  const running = start(
    sui.sizedBox({ child: sui.scrollView(rows, { controller }), width: 300, height: 200 }),
  );
  running.render();
  assert.equal(controller.viewportSize.height, 200);
  assert.ok(controller.contentSize.height > 1000);
  assert.equal(controller.offset.y, 0);

  controller.scrollTo(undefined, 300);
  running.drain();
  const snapshot = running.render();
  assert.equal(controller.offset.y, 300);
  approx(snapshot.getOne({ text: "Row 0" }).bounds.y, -300);

  controller.scrollTo(undefined, 1e9);
  running.drain();
  running.render();
  assert.ok(controller.maxOffset.y > 0);
  assert.equal(controller.offset.y, controller.maxOffset.y);
});

test("scroll controller scrolls a virtual list to an item", () => {
  const controller = new sui.ScrollController();
  const items = Array.from({ length: 200 }, (_, i) => sui.label(`Item ${i}`));
  const running = start(
    sui.sizedBox({
      child: sui.virtualScrollView(items, { controller }),
      width: 300,
      height: 200,
    }),
  );
  assert.equal(running.render().find({ text: "Item 150" }).length, 0);

  controller.scrollToItem(150);
  running.drain();
  assert.ok(running.render().find({ text: "Item 150" }).length > 0);
});

test("label typography options", () => {
  const text = "A long sentence that would normally wrap inside a narrow column of text.";
  const running = start(
    sui.sizedBox({
      child: sui.column(
        [
          sui.label("Body", { semanticName: "body" }),
          sui.label("Title", { semanticName: "title", fontSize: 32, weight: 700 }),
          sui.label(text, { semanticName: "wrapped" }),
          sui.label(text, { semanticName: "single", singleLine: true }),
          sui.label("Copy me", {
            semanticName: "copy",
            selectable: true,
            color: sui.Color.rgba(1, 0, 0, 1),
          }),
        ],
        { alignItems: "stretch" },
      ),
      width: 160,
    }),
  );
  const snapshot = running.render();
  const height = (name) => snapshot.getOne({ name }).bounds.height;
  assert.ok(height("title") > 1.5 * height("body"));
  assert.ok(height("wrapped") > 2 * height("body"));
  approx(height("single"), height("body"));
  assert.ok(snapshot.getOne({ name: "copy" }));
});

test("listView accepts rich list items", () => {
  const changes = [];
  const running = start(
    sui.listView(
      "Files",
      [
        "plain.txt",
        new sui.ListItem("report.pdf", "2 MB", "Today", "file"),
        new sui.ListItem("locked.db", undefined, undefined, undefined, "Locked database", undefined, false),
      ],
      { onChange: (index, label) => changes.push([index, label]) },
    ),
  );
  const all = running.render().find().map((node) => node.name);
  assert.ok(all.includes("plain.txt"));
  assert.ok(all.some((name) => name?.includes("report.pdf")));
  assert.ok(all.includes("Locked database"));

  assert.throws(() => sui.listView("Bad", [42]), /ListItem/);
  assert.equal(new sui.ListItem("x").label, "x");
});

test("listView rejects other item types as an invalid argument", () => {
  assert.throws(() => sui.listView("Bad", [42]), { code: "InvalidArg" });
});

test("grid tracks and cells", () => {
  const running = start(
    sui.sizedBox({
      child: sui.grid(
        [
          sui.gridCell(sui.button("Header"), 0, 0, { columnSpan: 3 }),
          sui.gridCell(sui.button("Side"), 1, 0),
          sui.gridCell(sui.button("Main"), 1, 1, { columnSpan: 2 }),
        ],
        { columns: [100, "1fr", "minmax(50, 1fr)"], rows: ["auto", 200] },
      ),
      width: 500,
      height: 400,
    }),
  );
  const snapshot = running.render();
  const header = bounds(snapshot, "button", "Header");
  const side = bounds(snapshot, "button", "Side");
  const main = bounds(snapshot, "button", "Main");
  approx(header.width, 500);
  approx(side.width, 100);
  approx(main.x, 100);
  approx(main.width, 400);
  approx(side.height, 200);

  assert.throws(() => sui.grid([], { columns: ["wide"] }), /unknown grid track/);
  // A count still means that many equal columns.
  assert.ok(sui.grid([sui.label("a")], { columns: 3 }));
});

test("focus controller moves focus into its scope", () => {
  const controller = new sui.FocusController();
  const running = start(
    sui.column([
      sui.button("Elsewhere"),
      sui.focusScope(sui.textInput("Search"), { controller }),
    ]),
  );
  running.click(running.render().getOne({ role: "button", name: "Elsewhere" }));
  assert.equal(running.render().getOne({ role: "button", name: "Elsewhere" }).focused, true);

  controller.focus();
  running.drain();
  running.render();
  // Focus moves at the next frame.
  running.tick(1.0);
  running.drainReadyEvents();
  assert.equal(running.render().getOne({ role: "text_input", name: "Search" }).focused, true);
});
