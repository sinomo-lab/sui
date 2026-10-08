import pytest

import sinomo_ui as sui


@pytest.mark.parametrize("value", [3, 2.5, True, "text"])
def test_state_keeps_the_python_type(value):
    state = sui.State(value)
    assert state.get() == value
    assert type(state.get()) is type(value)


def test_state_rejects_unsupported_values():
    with pytest.raises(ValueError):
        sui.State([1, 2])


def test_select_and_watch_follow_updates():
    source = sui.State(2)
    doubled = source.select(lambda value: value * 2)
    seen = []
    subscription = source.watch(seen.append)

    source.set(3)
    assert doubled.get() == 6
    assert seen == [3]

    assert subscription.unsubscribe()
    source.set(4)
    assert doubled.get() == 8
    assert seen == [3]


def test_selection_writes_back_integers():
    selected = sui.State(1)
    app = sui.App()
    app.window(
        sui.Window("Tabs").root(
            sui.tabs("Sections", ["Design", "Inspect"], selected=selected)
        )
    )
    running = app.start()
    snapshot = running.render()

    running.press(snapshot.get_one(role="tabs"), "ArrowLeft")
    running.drain()

    assert selected.get() == 0
    assert type(selected.get()) is int
