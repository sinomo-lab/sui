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


def test_enabled_disables_controls_and_follows_state():
    enabled = sui.State(False)
    pressed = []
    running = start(
        sui.column(
            [
                sui.button("Save", on_press=lambda: pressed.append(True), enabled=enabled),
                sui.checkbox("Agree", enabled=False),
                sui.slider("Volume", enabled=False),
                sui.text_input("Name", enabled=False),
            ]
        )
    )
    snapshot = running.render()
    assert snapshot.get_one(role="button", name="Save").disabled
    assert snapshot.get_one(role="checkbox").disabled
    assert snapshot.get_one(role="slider").disabled
    # Like a test locator, clicking refuses a control that cannot act.
    with pytest.raises(RuntimeError, match="not actionable"):
        running.click(snapshot.get_one(role="button", name="Save"))
    assert pressed == []

    enabled.set(True)
    running.drain()
    snapshot = running.render()
    save = snapshot.get_one(role="button", name="Save")
    assert not save.disabled
    running.click(save)
    assert pressed == [True]


def bounds(snapshot, role, name):
    return snapshot.get_one(role=role, name=name).bounds


def test_spacer_and_flex_items_share_the_main_axis():
    running = start(
        sui.column(
            [
                sui.row([sui.label("Title"), sui.spacer(), sui.button("Save")]),
                sui.row([sui.flex_item(sui.button("Grow"), grow=1), sui.button("Fixed")]),
            ],
            # A column sizes its cross axis to its content unless it stretches.
            align_items="stretch",
        )
    )
    snapshot = running.render()

    assert bounds(snapshot, "button", "Save").x + bounds(snapshot, "button", "Save").width > 600
    grow = bounds(snapshot, "button", "Grow")
    fixed = bounds(snapshot, "button", "Fixed")
    assert grow.width > 3 * fixed.width
    assert fixed.x + fixed.width > 600


def test_column_justify_and_alignment():
    running = start(
        sui.sized_box(
            sui.column(
                [sui.button("A"), sui.button("Longer button")],
                justify="end",
                align_items="center",
            ),
            width=640,
            height=480,
        )
    )
    snapshot = running.render()
    short = bounds(snapshot, "button", "A")
    long = bounds(snapshot, "button", "Longer button")

    assert long.y + long.height > 440
    assert abs((short.x + short.width / 2) - (long.x + long.width / 2)) < 1


def test_flex_item_outside_a_flex_container_shows_its_child():
    running = start(sui.flex_item(sui.button("Alone"), grow=1))
    assert running.render().get_one(role="button", name="Alone")


def test_unknown_justify_is_rejected():
    with pytest.raises(ValueError, match="unknown justify"):
        sui.row([], justify="sideways")


def test_button_presentation_and_semantics_options():
    running = start(
        sui.row(
            [
                sui.button(
                    "Delete",
                    appearance="filled",
                    tone="danger",
                    icon="close",
                    min_width=160,
                    semantic_name="Delete project",
                    description="Removes the project permanently",
                ),
                sui.button("Cancel", appearance="ghost"),
            ]
        )
    )
    snapshot = running.render()
    delete = snapshot.get_one(role="button", name="Delete project")
    assert delete.description == "Removes the project permanently"
    assert delete.bounds.width >= 160

    with pytest.raises(ValueError, match="unknown button appearance"):
        sui.button("Bad", appearance="glossy")


def test_text_field_submit_focus_and_read_only():
    submitted = []
    focus = []
    running = start(
        sui.column(
            [
                sui.text_input(
                    "Search",
                    on_submit=submitted.append,
                    on_focus_change=focus.append,
                ),
                sui.text_area("Notes", value="Fixed", read_only=True),
            ]
        )
    )
    snapshot = running.render()
    search = snapshot.get_one(role="text_input", name="Search")
    running.fill(search, "sui")
    running.press(running.render().get_one(role="text_input", name="Search"), "Enter")

    assert submitted == ["sui"]
    assert focus and focus[0] is True
    notes = running.render().get_one(name="Notes")
    assert not notes.editable


def test_semantic_name_overrides_the_visible_text():
    running = start(
        sui.column(
            [
                sui.checkbox("On", semantic_name="Enable sync"),
                sui.switch("On", semantic_name="Dark mode"),
                sui.radio_button("A", semantic_name="Option A"),
                sui.label("42%", semantic_name="Battery level"),
            ]
        )
    )
    snapshot = running.render()
    assert snapshot.get_one(role="checkbox").name == "Enable sync"
    assert snapshot.get_one(role="switch").name == "Dark mode"
    assert snapshot.get_one(role="radio_button").name == "Option A"
    assert snapshot.find(name="Battery level")


def test_rebuild_on_change_rebuilds_the_subtree():
    count = sui.State(1)
    title = sui.State("Draft")
    builds = []

    def build():
        builds.append(count.get())
        return sui.column(
            [sui.label(title)] + [sui.label(f"Item {i}") for i in range(count.get())]
        )

    running = start(sui.rebuild_on_change([count], build))
    snapshot = running.render()
    assert snapshot.find(text="Item 0") and not snapshot.find(text="Item 1")

    count.set(3)
    running.drain()
    snapshot = running.render()
    assert snapshot.find(text="Item 2")
    assert builds == [1, 3]

    # States used inside the rebuilt subtree stay live.
    title.set("Final")
    running.drain()
    assert running.render().find(text="Final")
    assert builds == [1, 3]


def test_rebuild_on_change_reports_builder_errors(exceptions):
    broken = sui.State(False)

    def build():
        if broken.get():
            raise RuntimeError("cannot build")
        return sui.label("Fine")

    running = start(sui.rebuild_on_change([broken], build))
    assert running.render().find(text="Fine")

    broken.set(True)
    running.drain()
    assert not running.render().find(text="Fine")
    assert [str(error) for error in exceptions] == ["cannot build"]


def test_scroll_controller_scrolls_and_reports_metrics():
    controller = sui.ScrollController()
    rows = sui.column([sui.label(f"Row {i}") for i in range(100)], align_items="stretch")
    running = start(
        sui.sized_box(sui.scroll_view(rows, controller=controller), width=300, height=200)
    )
    running.render()
    assert controller.viewport_size.height == 200
    assert controller.content_size.height > 1000
    assert controller.offset.y == 0

    controller.scroll_to(y=300)
    running.drain()
    snapshot = running.render()
    assert controller.offset.y == 300
    assert snapshot.get_one(text="Row 0").bounds.y == pytest.approx(-300)

    controller.scroll_to(y=1e9)
    running.drain()
    running.render()
    assert controller.offset.y == controller.max_offset.y > 0


def test_scroll_controller_scrolls_a_virtual_list_to_an_item():
    controller = sui.ScrollController()
    items = [sui.label(f"Item {i}") for i in range(200)]
    running = start(
        sui.sized_box(
            sui.virtual_scroll_view(items, controller=controller), width=300, height=200
        )
    )
    assert not running.render().find(text="Item 150")

    controller.scroll_to_item(150)
    running.drain()
    assert running.render().find(text="Item 150")
