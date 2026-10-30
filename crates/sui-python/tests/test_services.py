import pytest

import sinomo_ui as sui


@pytest.fixture
def clipboard():
    """Restore the clipboard, which may be the user's system clipboard."""
    previous = sui.clipboard_text()
    yield
    sui.set_clipboard_text(previous or "")


def test_clipboard_round_trip(clipboard):
    sui.set_clipboard_text("sui clipboard test")
    assert sui.clipboard_text() == "sui clipboard test"


def test_widget_clipboard_matches_the_module_functions(clipboard):
    copied = []

    class Copier:
        def measure(self, constraints):
            return constraints.clamp(sui.Size(40, 20))

        def event_with_context(self, event, context):
            if event.kind == "pointer" and event.action == "down":
                context.set_clipboard_text("from a widget")
                copied.append(True)

    app = sui.App()
    app.window(sui.Window("Clipboard").root(sui.Widget(Copier())))
    running = app.start()
    running.render()
    running.handle_event(sui.Event.pointer("down", sui.Point(10, 10)))

    assert copied == [True]
    assert sui.clipboard_text() == "from a widget"


def test_open_url_refuses_urls_outside_the_allowed_schemes():
    # Allowed URLs would launch the real browser, so only refusals are tested.
    assert sui.open_url("file:///etc/passwd") is False
    assert sui.open_url("javascript:alert(1)") is False
    assert sui.open_url("docs/intro.md") is False


@pytest.fixture
def url_policy():
    """Restore the default URL policy."""
    yield
    sui.set_url_policy(None)


def test_url_policy_decides_what_opens(url_policy):
    asked = []

    def refuse_all(url):
        asked.append(url)
        return False

    assert sui.set_url_policy(refuse_all) is None
    assert sui.open_url("  https://example.com  ") is False
    assert asked == ["https://example.com"]

    assert sui.set_url_policy(None) is refuse_all
    assert sui.open_url("file:///etc/passwd") is False
    assert asked == ["https://example.com"]


def test_url_policy_must_be_callable_and_return_a_bool(url_policy):
    with pytest.raises(TypeError, match="callable"):
        sui.set_url_policy("https")
    sui.set_url_policy(lambda url: "yes")
    with pytest.raises(TypeError, match="must return a bool"):
        sui.open_url("https://example.com")


def test_url_allowed_by_default_extends_into_custom_policies():
    assert sui.url_allowed_by_default("https://example.com")
    assert sui.url_allowed_by_default("mailto:team@example.com")
    assert not sui.url_allowed_by_default("myapp://settings")
    assert not sui.url_allowed_by_default("docs/intro.md")


def test_open_url_can_be_a_link_on_open_callback(exceptions):
    # The policy refuses `file:`, so the click opens nothing.
    app = sui.App()
    app.window(
        sui.Window("Links").root(
            sui.link("Passwords", "file:///etc/passwd", on_open=sui.open_url)
        )
    )
    running = app.start()
    running.click(running.render().get_one(role="link", name="Passwords"))

    assert exceptions == []


def test_file_dialog_rejects_unknown_modes():
    app = sui.App()
    app.window(sui.Window("Dialogs").root(sui.label("x")))
    ui = app.start().ui_handle()
    with pytest.raises(ValueError, match="unknown file dialog mode"):
        ui.show_file_dialog(print, mode="sideways")


def start_ui():
    app = sui.App()
    app.window(sui.Window("Timers").root(sui.label("x")))
    running = app.start()
    return running, running.ui_handle()


def test_call_later_runs_once_on_the_ui_queue():
    import time

    running, ui = start_ui()
    fired = []
    timer = ui.call_later(0.02, lambda: fired.append(True))
    assert timer.active
    time.sleep(0.2)
    running.drain()
    assert fired == [True]
    assert not timer.active


def test_call_every_repeats_until_cancelled():
    import time

    running, ui = start_ui()
    fired = []
    timer = ui.call_every(0.02, lambda: fired.append(True))
    for _ in range(3):
        time.sleep(0.1)
        running.drain()
    assert len(fired) >= 2
    timer.cancel()
    count = len(fired)
    time.sleep(0.1)
    running.drain()
    assert len(fired) == count
    assert not timer.active


def test_cancelled_call_later_never_runs():
    import time

    running, ui = start_ui()
    fired = []
    ui.call_later(0.05, lambda: fired.append(True)).cancel()
    time.sleep(0.15)
    running.drain()
    assert fired == []


def test_timer_arguments_are_validated():
    _, ui = start_ui()
    with pytest.raises(ValueError):
        ui.call_later(-1, print)
    with pytest.raises(ValueError, match="greater than zero"):
        ui.call_every(0, print)
