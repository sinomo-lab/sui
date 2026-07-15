"use strict";

const assert = require("node:assert/strict");
const fs = require("node:fs");
const Module = require("node:module");
const path = require("node:path");
const vm = require("node:vm");

const workspace = path.resolve(__dirname, "../../..");
const platformLibrary = {
  darwin: "libsui_js.dylib",
  linux: "libsui_js.so",
  win32: "sui_js.dll",
}[process.platform];

if (!platformLibrary) {
  throw new Error(`Unsupported native test platform: ${process.platform}`);
}

const nativePath = process.env.SUI_JS_NATIVE_PATH
  ? path.resolve(process.env.SUI_JS_NATIVE_PATH)
  : path.join(workspace, "target", "debug", platformLibrary);

if (!fs.existsSync(nativePath)) {
  throw new Error(
    `Native SUI binding not found at ${nativePath}; run cargo build --package sinomo-ui-js`,
  );
}

const nativeModule = new Module(nativePath, module);
nativeModule.filename = nativePath;
process.dlopen(nativeModule, nativePath);
const sui = nativeModule.exports;

function runExample(name) {
  const filename = path.join(__dirname, "..", "examples", name);
  const source = fs.readFileSync(filename, "utf8");
  vm.runInNewContext(source, {
    Buffer,
    console,
    require(specifier) {
      assert.equal(specifier, "..");
      return sui;
    },
  }, { filename });
}

function start(widget, title) {
  const window = new sui.Window(title);
  window.root(widget);
  const app = new sui.App();
  app.window(window);
  const running = app.start();
  running.render();
  return running;
}

function click(running, x, y) {
  running.handleEvent(
    sui.Event.pointer("down", new sui.Point(x, y), undefined, undefined, "primary", 1)
  );
  running.handleEvent(
    sui.Event.pointer("up", new sui.Point(x, y), undefined, undefined, "primary")
  );
}

runExample("counter.js");
runExample("custom-widget.js");

const text = new sui.State("Ready");
const textRunning = start(sui.Label(text), "State text");
text.set("Updated");
assert.equal(textRunning.pendingCount, 1);
assert.equal(textRunning.drain(), 1);
assert.equal(text.get(), "Updated");

const checked = new sui.State(false);
let toggled;
const checkboxRunning = start(
  sui.Checkbox("Enabled", checked, (value) => {
    toggled = value;
  }),
  "State boolean"
);
click(checkboxRunning, 32, 18);
assert.equal(checked.get(), true);
assert.equal(toggled, true);
assert.equal(Array.isArray(toggled), false);

const selected = new sui.State(0);
let selectedIndex;
let selectedValue;
const radioRunning = start(
  sui.RadioGroup("Priority", ["Low", "Medium", "High"], selected, (index, value) => {
    selectedIndex = index;
    selectedValue = value;
  }),
  "State number"
);
click(radioRunning, 20, 52);
assert.equal(selected.get(), 1);
assert.equal(selectedIndex, 1);
assert.equal(selectedValue, "Medium");

const customCalls = new Set();
const custom = {
  name: "Consumer boundary",
  measure(constraints) {
    assert.equal(this, custom);
    assert.equal(constraints instanceof sui.Constraints, true);
    customCalls.add("measure");
    return constraints.clamp(new sui.Size(80, 24));
  },
  event(event) {
    assert.equal(this, custom);
    assert.equal(event instanceof sui.Event, true);
    assert.equal(event.customKind, "consumer-probe");
    customCalls.add("event");
    return true;
  },
  paint(paint) {
    assert.equal(this, custom);
    assert.equal(paint instanceof sui.Paint, true);
    customCalls.add("paint");
    paint.fillRect(paint.bounds, new sui.Color(0.2, 0.4, 0.8, 1));
  },
  semantics(semantics) {
    assert.equal(this, custom);
    assert.equal(semantics instanceof sui.Semantics, true);
    customCalls.add("semantics");
    semantics.node("button", "Consumer boundary");
  },
};
const customSnapshot = sui.renderWidget(
  new sui.Widget(custom),
  sui.Event.custom("consumer-probe")
);
assert.equal(customSnapshot.commandCount > 0, true);
assert.deepEqual([...customCalls].sort(), ["event", "measure", "paint", "semantics"]);

const modifiers = new sui.Modifiers(true, true, false, false);
const pointer = sui.Event.pointer(
  "move",
  new sui.Point(4, 5),
  "9",
  undefined,
  undefined,
  undefined,
  "mouse",
  true,
  modifiers
);
assert.equal(pointer.modifiers.shift, true);
assert.equal(pointer.modifiers.control, true);
const keyboard = sui.Event.keyboard("A", "pressed", "KeyA", "a", false, false, modifiers);
assert.equal(keyboard.modifiers.shift, true);
const ime = sui.Event.ime("compositionUpdate", "abc", 1, 2);
assert.equal(ime.cursorStart, 1);
assert.equal(ime.cursorEnd, 2);
const resized = sui.Event.window("resized", undefined, new sui.Size(640, 480));
assert.equal(resized.size.width, 640);
const scale = sui.Event.window(
  "scaleFactorChanged",
  undefined,
  undefined,
  2,
  192,
  new sui.Size(800, 600)
);
assert.equal(scale.scaleFactor, 2);
assert.equal(scale.rawDpi, 192);
assert.equal(scale.suggestedSize.height, 600);

sui.Shader.saturationValuePlane(0.3, 1, "srgb");
sui.Shader.saturationBar(0.3, 0.8, "srgb");
sui.Shader.valueBar(0.3, 0.8, 1, "srgb");
sui.Shader.alphaBar(new sui.Color(1, 0, 0, 1));
sui.Shader.rgbChannelBar(new sui.Color(1, 0, 0, 1), 0, 1);

assert.equal(new sui.ExternalBackendHandle("0").isEmpty, true);
assert.equal(sui.ExternalSync.generation("7").value, "7");

const mutablePoint = new sui.Point(1, 2);
mutablePoint.x = 3;
assert.equal(mutablePoint.x, 3);

const portableExports = [
  "ActionCard",
  "BrushPreview",
  "CommandGroup",
  "CoverageDots",
  "DateTimeInput",
  "Dock",
  "FixedPaneSplit",
  "FramedField",
  "MeasuredBottomDock",
  "PasswordInput",
  "PlacementBadge",
  "PropertyRow",
  "SectionLabel",
  "SideSheet",
  "SplitView",
  "SwitchView",
  "TrailingSlotRow",
];
for (const name of portableExports) {
  assert.equal(typeof sui[name], "function", `${name} should be exported`);
}

const brushColor = new sui.Color(0.2, 0.5, 0.9, 1);
const brushSpec = new sui.BrushPreviewSpec(brushColor, 20, 0.6, "square");
assert.equal(brushSpec.color.blue, 0.9);
assert.equal(brushSpec.size, 20);
assert.equal(brushSpec.opacity, 0.6);
assert.equal(brushSpec.shape, "square");

const portableWidgets = [
  sui.ActionCard(
    "Open project",
    "Choose a recent workspace",
    undefined,
    undefined,
    new sui.State(true),
    () => {}
  ),
  sui.BrushPreview("Brush", brushSpec, undefined, new sui.Size(40, 40)),
  sui.CommandGroup(
    "Commands",
    [sui.Button("Run"), sui.Button("Stop")],
    "vertical",
    8,
    4,
    6,
    new sui.Color(0.1, 0.1, 0.1, 1),
    new sui.Color(0.3, 0.3, 0.3, 1)
  ),
  sui.CoverageDots("Coverage", 2, 4, "accent", 4, true, 80),
  sui.DateTimeInput(
    "Appointment",
    new sui.State("2026-07-15T12:30"),
    "YYYY-MM-DD HH:mm",
    () => {}
  ),
  sui.Dock(
    sui.Label("Dock body"),
    sui.Label("Dock top"),
    24,
    sui.Label("Dock bottom"),
    24,
    320,
    240
  ),
  sui.FixedPaneSplit(
    sui.Label("Fixed"),
    sui.Label("Divider"),
    sui.Label("Flexible"),
    "horizontal",
    "first",
    96,
    1,
    240
  ),
  sui.FramedField(
    sui.TextInput("Framed input"),
    "Framed input",
    "A field with shared chrome",
    8,
    36,
    true,
    new sui.State(false),
    new sui.State(false)
  ),
  sui.MeasuredBottomDock(
    sui.Label("Measured body"),
    sui.Label("Measured bottom"),
    new sui.Size(640, 640)
  ),
  sui.PasswordInput("Password", new sui.State("secret"), "Enter password", () => {}),
  sui.PlacementBadge(new sui.State("Primary"), undefined, "accent", 2, 4, 96),
  sui.PropertyRow("Opacity", sui.Label("100%"), true, 96, 180, 8),
  sui.SectionLabel("Appearance", "Appearance section", brushColor),
  sui.SideSheet(
    "Inspector",
    sui.Label("Inspector body"),
    "Edit the current selection",
    new sui.State(true),
    true,
    true,
    "right",
    280,
    sui.Button("Help"),
    [sui.Button("Apply")],
    () => {}
  ),
  sui.SplitView(
    sui.Label("First pane"),
    sui.Label("Second pane"),
    "horizontal",
    "Workspace split",
    new sui.State(0.5),
    120,
    120,
    1,
    () => {}
  ),
  sui.SwitchView([sui.Label("First view"), sui.Label("Second view")], new sui.State(1)),
  sui.TrailingSlotRow(sui.Label("Row body"), sui.Button("Edit"), 64, 28, 8),
];

assert.equal(portableWidgets.length, portableExports.length);
for (const [index, widget] of portableWidgets.entries()) {
  assert.equal(widget instanceof sui.Widget, true, `${portableExports[index]} should return Widget`);
  const snapshot = sui.renderWidget(widget);
  assert.equal(
    Number.isInteger(snapshot.commandCount),
    true,
    `${portableExports[index]} should render through the native addon`
  );
}

const auditExports = [
  "FloatingStackWindow",
  "FloatingStack",
  "VirtualScrollView",
  "ReorderableList",
];
for (const name of auditExports) {
  assert.equal(typeof sui[name], "function", `${name} should be exported`);
}

const floatingWindow = new sui.FloatingStackWindow(
  new sui.Rect(12, 16, 180, 80),
  sui.Label("Floating window")
);
assert.equal(floatingWindow instanceof sui.FloatingStackWindow, true);

const auditWidgets = [
  sui.FloatingStack([floatingWindow], "Floating workspace"),
  sui.VirtualScrollView(
    [sui.Label("Virtual row one"), sui.Label("Virtual row two")],
    "Virtual results",
    8,
    4
  ),
  sui.ReorderableList(
    "Layers",
    [sui.Label("Background"), sui.Label("Foreground")],
    8,
    4,
    "Moving layer",
    (item, from, to) => {
      assert.equal(Array.isArray(item), false);
      assert.equal(typeof from, "number");
      assert.equal(typeof to, "number");
    }
  ),
];

for (const [index, widget] of auditWidgets.entries()) {
  const exportName = auditExports[index + 1];
  assert.equal(widget instanceof sui.Widget, true, `${exportName} should return Widget`);
  const snapshot = sui.renderWidget(widget);
  assert.equal(
    Number.isInteger(snapshot.commandCount),
    true,
    `${exportName} should render through the native addon`
  );
}

let reorderArgs;
const reorderRunning = start(
  sui.ReorderableList(
    "Reorder callback",
    [
      sui.SizedBox(sui.Label("First"), 200, 40),
      sui.SizedBox(sui.Label("Second"), 200, 40),
      sui.SizedBox(sui.Label("Third"), 200, 40),
    ],
    8,
    4,
    "Moving row",
    (item, from, to) => {
      reorderArgs = [item, from, to];
    }
  ),
  "Reorder callback"
);
reorderRunning.handleEvent(
  sui.Event.pointer("down", new sui.Point(20, 20), "51", undefined, "primary", 1)
);
reorderRunning.handleEvent(
  sui.Event.pointer("move", new sui.Point(20, 30), "51", undefined, undefined, 1)
);
reorderRunning.handleEvent(
  sui.Event.pointer("move", new sui.Point(20, 120), "51", undefined, undefined, 1)
);
reorderRunning.handleEvent(
  sui.Event.pointer("up", new sui.Point(20, 120), "51", undefined, "primary", 0)
);
assert.deepEqual(reorderArgs, [0, 0, 2]);
assert.equal(Array.isArray(reorderArgs[0]), false);

console.log("sui-js consumer boundary: ok");
