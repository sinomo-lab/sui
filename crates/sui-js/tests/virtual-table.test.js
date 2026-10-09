// The virtual table: a keyed row model, selection, activation, and headers.
import assert from "node:assert/strict";
import { setTimeout as delay } from "node:timers/promises";
import { test } from "node:test";
import { sui } from "./support.js";

function start(root) {
  // A sized window gets a viewport, as a platform host would provide.
  const window = new sui.Window("Table").setInitialSize(new sui.Size(640, 480)).root(root);
  return new sui.App().window(window).start();
}

const rows = (snapshot) => snapshot.find({ role: "list_item" });
const rowNames = (snapshot) => rows(snapshot).map((node) => node.name);
const row = (snapshot, name) => snapshot.getOne({ role: "list_item", name });

function makeRows(count) {
  return Array.from({ length: count }, (_, i) => {
    const key = i + 1;
    return new sui.VirtualTableRow(String(key), [`Row ${key}`, String(key * 10)]);
  });
}

const COLUMNS = [
  new sui.VirtualTableColumn("1", "Name", 200),
  new sui.VirtualTableColumn("2", "Value", undefined, undefined, undefined, undefined, "end"),
];

function pointer(running, action, position, buttons) {
  running.handleEvent(
    sui.Event.pointer(action, position, undefined, undefined, "primary", buttons),
  );
}

test("table model mutates rows by key", () => {
  const model = new sui.TableModel(makeRows(3));
  assert.equal(model.size, 3);
  assert.equal(model.insert(0, new sui.VirtualTableRow("9", ["Nine", "90"])), true);
  assert.deepEqual(model.get("9"), ["Nine", "90"]);
  assert.equal(model.update(new sui.VirtualTableRow("2", ["Two", "20"])), true);
  assert.equal(model.update(new sui.VirtualTableRow("42", ["Missing"])), false);
  assert.equal(model.remove("9"), true);
  assert.equal(model.remove("9"), false);
  assert.equal(model.size, 3);
  assert.equal(model.setSort("1", "descending"), true);
  assert.equal(model.setSort(), true);

  assert.throws(() => new sui.VirtualTableRow("0", ["Zero"]));
  assert.throws(() => model.append(new sui.VirtualTableRow("1", ["Duplicate"])));
  assert.throws(
    () => new sui.TableModel([new sui.VirtualTableRow("5", ["A"]), new sui.VirtualTableRow("5", ["B"])]),
  );
  assert.throws(() => model.setSort("1", "sideways"));

  const column = new sui.VirtualTableColumn(
    "7", "Size", undefined, undefined, undefined, undefined, undefined, "ascending",
  );
  assert.deepEqual([column.key, column.title], ["7", "Size"]);
});

test("a large model realizes only visible rows", () => {
  const model = new sui.TableModel(makeRows(10_000));
  const started = performance.now();
  const running = start(sui.virtualTable("Files", COLUMNS, model));
  const snapshot = running.render();
  const elapsed = performance.now() - started;

  const names = rowNames(snapshot);
  assert.equal(names[0], "Row 1");
  assert.ok(names.length > 0 && names.length < 40);
  assert.ok(!names.includes("Row 9000"));
  assert.equal(snapshot.getOne({ role: "table" }).value, "10000 rows");
  assert.ok(elapsed < 5000);
});

test("selection by click and keyboard writes the key", () => {
  const model = new sui.TableModel(makeRows(200));
  const selected = new sui.State(0);
  const changes = [];
  const running = start(
    sui.virtualTable("Files", COLUMNS, model, {
      selected,
      onChange: (key) => changes.push(key),
    }),
  );
  const snapshot = running.render();

  running.click(row(snapshot, "Row 3"));
  running.drain();
  assert.equal(selected.get(), 3);
  assert.deepEqual(changes, ["3"]);
  assert.equal(row(running.render(), "Row 3").selected, true);

  // `press` clicks the node first to focus it; the row stays selected.
  running.press(row(running.render(), "Row 3"), "ArrowDown");
  running.drain();
  assert.equal(selected.get(), 4);
  assert.deepEqual(changes, ["3", "4"]);

  // The bound state drives the selection from the application side.
  selected.set(7);
  running.drain();
  assert.equal(row(running.render(), "Row 7").selected, true);
});

test("a string selection state receives string keys", () => {
  const model = new sui.TableModel(makeRows(20));
  const selected = new sui.State("");
  const running = start(sui.virtualTable("Files", COLUMNS, model, { selected }));

  running.click(row(running.render(), "Row 3"));
  running.drain();
  assert.equal(selected.get(), "3");

  selected.set("5");
  running.drain();
  assert.equal(row(running.render(), "Row 5").selected, true);
});

test("row activation runs on Enter and double click", () => {
  const model = new sui.TableModel(makeRows(20));
  const activated = [];
  const running = start(
    sui.virtualTable("Files", COLUMNS, model, { onRowActivate: (key) => activated.push(key) }),
  );
  const target = row(running.render(), "Row 2");

  running.click(target);
  assert.deepEqual(activated, []);
  running.click(target);
  assert.deepEqual(activated, ["2"]);

  running.press(row(running.render(), "Row 2"), "Enter");
  assert.deepEqual(activated, ["2", "2"]);
});

test("model updates from async callbacks show after idle", async () => {
  const model = new sui.TableModel(makeRows(50));
  let update;
  const running = start(
    sui.column([
      sui.button("Refresh", { onPress: () => update() }),
      sui.sizedBox({ child: sui.virtualTable("Files", COLUMNS, model), width: 600, height: 400 }),
    ]),
  );
  assert.equal(rowNames(running.render())[0], "Row 1");

  // JavaScript has no worker threads sharing the model; updates made after
  // an await, or from a timer, are applied at the next drain.
  update = async () => {
    await delay(5);
    model.update(new sui.VirtualTableRow("1", ["Renamed", "1"]));
    model.remove("2");
  };
  running.click(running.render().getOne({ role: "button", name: "Refresh" }));
  await running.idle();
  await new Promise((resolve) =>
    setTimeout(() => {
      model.insert(0, new sui.VirtualTableRow("500", ["Inserted", "5"]));
      resolve();
    }, 5),
  );
  running.drain();

  const snapshot = running.render();
  assert.deepEqual(rowNames(snapshot).slice(0, 3), ["Inserted", "Renamed", "Row 3"]);
  assert.equal(snapshot.getOne({ role: "table" }).value, "50 rows");
});

test("header activation lets the app sort and keeps the selection", () => {
  const model = new sui.TableModel(makeRows(100));
  const selected = new sui.State(0);
  const headers = [];

  function sortBy(columnKey) {
    headers.push(columnKey);
    const keys = Array.from({ length: 100 }, (_, i) => String(100 - i));
    model.replace(keys.map((key) => new sui.VirtualTableRow(key, model.get(key))));
    model.setSort(columnKey, "descending");
  }

  const running = start(
    sui.virtualTable("Files", COLUMNS, model, { selected, onHeaderActivate: sortBy }),
  );
  const snapshot = running.render();
  running.click(row(snapshot, "Row 1"));
  running.drain();
  assert.equal(selected.get(), 1);

  const first = rows(snapshot)[0].bounds;
  const header = new sui.Point(first.x + 40, first.y - 18);
  pointer(running, "down", header, 1);
  pointer(running, "up", header);
  running.drain();

  assert.deepEqual(headers, ["1"]);
  const sorted = running.render();
  assert.equal(rowNames(sorted)[0], "Row 100");
  assert.equal(selected.get(), 1);
  assert.ok(!rows(sorted).some((node) => node.selected));
});

test("column resize reports the column key and width", () => {
  const model = new sui.TableModel(makeRows(10));
  const resized = [];
  const running = start(
    sui.virtualTable("Files", COLUMNS, model, {
      onColumnResize: (key, width) => resized.push([key, width]),
    }),
  );
  const first = rows(running.render())[0].bounds;
  const edge = new sui.Point(first.x + 200, first.y - 18);
  const target = new sui.Point(first.x + 260, first.y - 18);
  pointer(running, "down", edge, 1);
  pointer(running, "move", target, 1);
  pointer(running, "up", target);

  assert.ok(resized.length > 0);
  const [key, width] = resized.at(-1);
  assert.equal(key, "1");
  assert.ok(Math.abs(width - 260) <= 1);
});

test("a disabled table ignores input", () => {
  const model = new sui.TableModel(makeRows(10));
  const selected = new sui.State(0);
  const running = start(
    sui.virtualTable("Files", COLUMNS, model, { selected, enabled: false }),
  );
  running.click(row(running.render(), "Row 2"));
  running.drain();
  assert.equal(selected.get(), 0);
});
