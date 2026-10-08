import pytest

import sinomo_ui as sui


def start(root):
    app = sui.App()
    # A sized window gets a viewport, as a platform host would provide.
    app.window(sui.Window("Widgets", size=sui.Size(640, 480)).root(root))
    return app.start()


def names(snapshot, role):
    return [node.name for node in snapshot.find(role=role)]


def test_state_bound_labels_update_in_place():
    label = sui.State("Save")
    running = start(
        sui.column(
            [
                sui.button(label),
                sui.checkbox(label),
                sui.switch(label),
                sui.radio_button(label),
            ]
        )
    )
    assert names(running.render(), "button") == ["Save"]

    label.set("Saved")
    running.drain()
    snapshot = running.render()

    assert names(snapshot, "button") == ["Saved"]
    assert names(snapshot, "checkbox") == ["Saved"]


def test_dialog_follows_state_and_clears_it_on_dismiss():
    shown = sui.State(False)
    dismissed = []
    running = start(
        sui.dialog(
            "Confirm",
            sui.label("Apply the change?"),
            open=shown,
            actions=[sui.button("Apply")],
            on_dismiss=lambda: dismissed.append(True),
        )
    )
    assert not running.render().find(role="dialog")

    shown.set(True)
    running.drain()
    snapshot = running.render()
    assert snapshot.find(role="dialog")
    assert "Apply" in names(snapshot, "button")

    running.press(snapshot.get_one(role="dialog"), "Escape")
    running.drain()
    assert shown.get() is False
    assert dismissed == [True]
    assert not running.render().find(role="dialog")


def test_tabs_show_panels_and_report_changes():
    selected = sui.State(0)
    changes = []
    running = start(
        sui.tabs(
            "Sections",
            ["Design", "Inspect"],
            selected=selected,
            panels=[sui.label("Design body"), sui.label("Inspect body")],
            on_change=lambda index, label: changes.append((index, label)),
        )
    )
    assert running.render().find(text="Design body")

    selected.set(1)
    running.drain()
    snapshot = running.render()
    assert snapshot.find(text="Inspect body")

    running.press(snapshot.get_one(role="tabs"), "ArrowLeft")
    running.drain()
    assert selected.get() == 0
    assert changes == [(0, "Design")]


def test_tabs_reject_mismatched_panels():
    with pytest.raises(ValueError, match="one panel per tab"):
        sui.tabs("Bad", ["One", "Two"], panels=[sui.label("Only one")])


def test_popover_follows_state():
    shown = sui.State(False)
    running = start(
        sui.popover("Details", sui.button("More"), sui.label("Popover body"), open=shown)
    )
    assert not running.render().find(text="Popover body")

    shown.set(True)
    running.drain()
    assert running.render().find(text="Popover body")
