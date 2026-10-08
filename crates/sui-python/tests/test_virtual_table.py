import threading
import time

import pytest

import sinomo_ui as sui


def start(root):
    app = sui.App()
    # A sized window gets a viewport, as a platform host would provide.
    app.window(sui.Window("Table", size=sui.Size(640, 480)).root(root))
    return app.start()


def rows(snapshot):
    return snapshot.find(role="list_item")


def row_names(snapshot):
    return [node.name for node in rows(snapshot)]


def make_rows(count):
    return [sui.VirtualTableRow(key, [f"Row {key}", str(key * 10)]) for key in range(1, count + 1)]


COLUMNS = [
    sui.VirtualTableColumn(1, "Name", width=200),
    sui.VirtualTableColumn(2, "Value", alignment="end"),
]


def test_table_model_mutates_rows_by_key():
    model = sui.TableModel(make_rows(3))
    assert model.size == 3
    assert model.insert(0, sui.VirtualTableRow(9, ["Nine", "90"]))
    assert model.get(9) == ["Nine", "90"]
    assert model.update(sui.VirtualTableRow(2, ["Two", "20"]))
    assert not model.update(sui.VirtualTableRow(42, ["Missing"]))
    assert model.remove(9)
    assert not model.remove(9)
    assert model.size == 3
    assert model.set_sort(1, "descending")
    assert model.set_sort()

    with pytest.raises(ValueError):
        sui.VirtualTableRow(0, ["Zero"])
    with pytest.raises(ValueError):
        model.append(sui.VirtualTableRow(1, ["Duplicate"]))
    with pytest.raises(ValueError):
        sui.TableModel([sui.VirtualTableRow(5, ["A"]), sui.VirtualTableRow(5, ["B"])])
    with pytest.raises(ValueError):
        model.set_sort(1, "sideways")

    column = sui.VirtualTableColumn(7, "Size", sort_direction="ascending")
    assert (column.key, column.title) == (7, "Size")


def test_large_model_realizes_only_visible_rows():
    model = sui.TableModel(make_rows(10_000))
    started = time.perf_counter()
    running = start(sui.virtual_table("Files", COLUMNS, model))
    snapshot = running.render()
    elapsed = time.perf_counter() - started

    names = row_names(snapshot)
    assert names[0] == "Row 1"
    assert 0 < len(names) < 40
    assert "Row 9000" not in names
    assert snapshot.get_one(role="table").value == "10000 rows"
    assert elapsed < 5.0


def test_selection_by_click_and_keyboard_writes_the_key():
    model = sui.TableModel(make_rows(200))
    selected = sui.State(0)
    changes = []
    running = start(
        sui.virtual_table(
            "Files",
            COLUMNS,
            model,
            selected=selected,
            on_change=changes.append,
        )
    )
    snapshot = running.render()

    running.click(snapshot.get_one(role="list_item", name="Row 3"))
    running.drain()
    assert selected.get() == 3
    assert changes == [3]
    assert snapshot.get_one(role="list_item", name="Row 3")
    assert running.render().get_one(role="list_item", name="Row 3").selected

    # `press` clicks the node first to focus it; the row stays selected.
    running.press(running.render().get_one(role="list_item", name="Row 3"), "ArrowDown")
    running.drain()
    assert selected.get() == 4
    assert changes == [3, 4]

    # The bound state drives the selection from the application side.
    selected.set(7)
    running.drain()
    assert running.render().get_one(role="list_item", name="Row 7").selected


def test_row_activation_runs_on_enter_and_double_click():
    model = sui.TableModel(make_rows(20))
    activated = []
    running = start(
        sui.virtual_table("Files", COLUMNS, model, on_row_activate=activated.append)
    )
    snapshot = running.render()
    row = snapshot.get_one(role="list_item", name="Row 2")

    running.click(row)
    assert activated == []
    running.click(row)
    assert activated == [2]

    running.press(running.render().get_one(role="list_item", name="Row 2"), "Enter")
    assert activated == [2, 2]


def test_model_updates_from_another_thread_show_after_drain():
    model = sui.TableModel(make_rows(50))
    running = start(sui.virtual_table("Files", COLUMNS, model))
    assert row_names(running.render())[0] == "Row 1"

    def work():
        model.update(sui.VirtualTableRow(1, ["Renamed", "1"]))
        model.remove(2)
        model.insert(0, sui.VirtualTableRow(500, ["Inserted", "5"]))

    worker = threading.Thread(target=work)
    worker.start()
    worker.join()
    running.drain()

    snapshot = running.render()
    assert row_names(snapshot)[:3] == ["Inserted", "Renamed", "Row 3"]
    assert snapshot.get_one(role="table").value == "50 rows"


def test_header_activation_lets_the_app_sort_and_keeps_the_selection():
    model = sui.TableModel(make_rows(100))
    selected = sui.State(0)
    headers = []

    def sort_by(column_key):
        headers.append(column_key)
        keys = sorted(range(1, 101), reverse=True)
        model.replace([sui.VirtualTableRow(key, model.get(key)) for key in keys])
        model.set_sort(column_key, "descending")

    running = start(
        sui.virtual_table(
            "Files",
            COLUMNS,
            model,
            selected=selected,
            on_header_activate=sort_by,
        )
    )
    snapshot = running.render()
    running.click(snapshot.get_one(role="list_item", name="Row 1"))
    running.drain()
    assert selected.get() == 1

    first = rows(snapshot)[0].bounds
    header = sui.Point(first.x + 40, first.y - 18)
    running.handle_event(sui.Event.pointer("down", header, button="primary", buttons=1))
    running.handle_event(sui.Event.pointer("up", header, button="primary"))
    running.drain()

    assert headers == [1]
    sorted_snapshot = running.render()
    assert row_names(sorted_snapshot)[0] == "Row 100"
    assert selected.get() == 1
    assert not any(node.selected for node in rows(sorted_snapshot))


def test_column_resize_reports_the_column_key_and_width():
    model = sui.TableModel(make_rows(10))
    resized = []
    running = start(
        sui.virtual_table(
            "Files",
            COLUMNS,
            model,
            on_column_resize=lambda key, width: resized.append((key, width)),
        )
    )
    first = rows(running.render())[0].bounds
    edge = sui.Point(first.x + 200, first.y - 18)
    target = sui.Point(first.x + 260, first.y - 18)
    running.handle_event(sui.Event.pointer("down", edge, button="primary", buttons=1))
    running.handle_event(sui.Event.pointer("move", target, button="primary", buttons=1))
    running.handle_event(sui.Event.pointer("up", target, button="primary"))

    assert resized and resized[-1][0] == 1
    assert resized[-1][1] == pytest.approx(260, abs=1)


def test_disabled_table_ignores_input():
    model = sui.TableModel(make_rows(10))
    selected = sui.State(0)
    running = start(
        sui.virtual_table("Files", COLUMNS, model, selected=selected, enabled=False)
    )
    running.click(running.render().get_one(role="list_item", name="Row 2"))
    running.drain()
    assert selected.get() == 0
