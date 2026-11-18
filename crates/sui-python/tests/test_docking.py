import pytest

import sinomo_ui as sui


def start(root):
    app = sui.App()
    app.window(sui.Window("Docking", size=sui.Size(900, 600)).root(root))
    return app.start()


def tabs_state(*ids):
    return sui.DockState(sui.DockLayout(sui.DockNode.tabs(list(ids))))


def closable(panel, title):
    return sui.DockPanelSpec(panel, title, sui.label(f"{title} panel"), closable=True)


def test_panel_options_describe_a_tab():
    options = sui.DockPanelOptions(
        "main.rs", icon="folder", closable=True, dirty=True, min_size=sui.Size(240, 120)
    )
    assert options.title == "main.rs"
    assert options.icon == "folder"
    assert options.closable and options.dirty
    assert options.min_size.width == 240

    plain = sui.DockPanelOptions("Output")
    assert plain.icon is None
    assert not plain.closable
    assert plain.min_size.height == 0

    spec = closable(1, "Files")
    assert spec.title == "Files"
    assert spec.options.closable

    with pytest.raises(ValueError):
        sui.DockPanelOptions("Bad", icon="not-an-icon")
    with pytest.raises(ValueError):
        sui.DockPanelOptions("Bad", min_size=sui.Size(-1, 0))
    with pytest.raises(ValueError):
        sui.DockPlacement.root("diagonal")


def test_panels_are_added_updated_reordered_and_removed_while_mounted():
    state = tabs_state(1)
    built = []

    def build(panel):
        built.append(panel)
        return sui.label(f"Built panel {panel}")

    running = start(
        sui.dock_workspace(
            state,
            [sui.DockPanelSpec(1, "Files", sui.label("Files panel"))],
            panel_builder=build,
        )
    )
    running.render()

    with pytest.raises(ValueError):
        state.add_panel(1, sui.DockPanelOptions("Files"))
    state.add_panel(2, sui.DockPanelOptions("Search"), sui.DockPlacement.beside(1, "right"))
    state.add_panel(3, sui.DockPanelOptions("Output"), sui.DockPlacement.hidden())
    state.add_panel(4, sui.DockPanelOptions("Notes"))
    assert state.is_registered(2)
    assert state.snapshot().hidden == [3]

    snapshot = running.render()
    assert built == [2, 3, 4]
    assert len(snapshot.find(name="Built panel 2")) == 1

    # Panel 4 joined panel 1's group; move it in front.
    assert state.dock_tab(4, 1, after=False)
    state.update_panel(4, sui.DockPanelOptions("Notes (edited)", dirty=True))
    snapshot = running.render()
    tab = snapshot.get_one(role="button", name="Notes (edited)")
    assert tab.bounds.x < snapshot.get_one(role="button", name="Files").bounds.x

    assert state.remove_panel(4)
    assert not state.remove_panel(4)
    assert not state.is_registered(4)
    with pytest.raises(ValueError):
        state.update_panel(4, sui.DockPanelOptions("Gone"))
    assert running.render().find(name="Notes (edited)") == []


def test_close_requests_hide_keep_or_remove_a_panel():
    state = tabs_state(1, 2, 3)
    actions = {1: "keep", 2: "remove", 3: None}
    asked = []

    def on_close_request(panel):
        asked.append(panel)
        return actions[panel]

    running = start(
        sui.dock_workspace(
            state,
            [closable(1, "One"), closable(2, "Two"), closable(3, "Three")],
            on_close_request=on_close_request,
        )
    )

    def close(title):
        running.click(running.render().get_one(role="button", name=f"Close {title}"))

    close("One")
    assert state.snapshot().hidden == []
    close("Two")
    assert not state.is_registered(2)
    close("Three")
    assert state.snapshot().hidden == [3]
    assert state.is_registered(3)
    assert asked == [1, 2, 3]


def test_a_failing_close_handler_keeps_the_panel_open(exceptions):
    state = tabs_state(1)

    def on_close_request(panel):
        raise ValueError("close failed")

    running = start(
        sui.dock_workspace(state, [closable(1, "One")], on_close_request=on_close_request)
    )
    running.click(running.render().get_one(role="button", name="Close One"))

    assert len(exceptions) == 1
    assert state.snapshot().hidden == []
    assert state.is_registered(1)


def test_retain_panels_prunes_a_saved_layout():
    saved = sui.DockLayout(
        sui.DockNode.split("horizontal", 0.3, sui.DockNode.tabs([1]), sui.DockNode.tabs([9])),
        [sui.DockFloatingGroup(1, [8], 8, sui.Rect(10, 10, 300, 200))],
        [7],
    )
    pruned = saved.retain_panels([1])
    assert pruned.floating == []
    assert pruned.hidden == []
    # The remaining layout is valid and holds only panel 1.
    state = sui.DockState(pruned)
    assert state.hide(1)
    with pytest.raises(ValueError):
        state.activate(9)
