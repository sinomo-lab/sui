import sys

import pytest

import sinomo_ui as sui


def start_with_buttons(**buttons):
    app = sui.App()
    app.window(
        sui.Window("Callbacks").root(
            sui.column(
                [sui.button(name, on_press=callback) for name, callback in buttons.items()]
            )
        )
    )
    running = app.start()
    return running, running.render()


def fail():
    raise ValueError("press failed")


def test_callback_exception_reaches_the_handler_with_traceback(exceptions):
    running, snapshot = start_with_buttons(Fail=fail)

    running.click(snapshot.get_one(role="button", name="Fail"))

    assert len(exceptions) == 1
    assert isinstance(exceptions[0], ValueError)
    assert exceptions[0].__traceback__ is not None


def test_default_report_uses_sys_excepthook(monkeypatch):
    reported = []
    monkeypatch.setattr(sys, "excepthook", lambda *info: reported.append(info))
    previous = sui.set_exception_handler(None)
    try:
        running, snapshot = start_with_buttons(Fail=fail)
        running.click(snapshot.get_one(role="button", name="Fail"))
    finally:
        sui.set_exception_handler(previous)

    assert len(reported) == 1
    error_type, error, traceback = reported[0]
    assert error_type is ValueError
    assert str(error) == "press failed"
    assert traceback is not None


def test_failing_handler_still_reports_the_original(monkeypatch):
    reported = []
    monkeypatch.setattr(sys, "excepthook", lambda *info: reported.append(info[0]))

    def broken_handler(error):
        raise RuntimeError("handler failed")

    previous = sui.set_exception_handler(broken_handler)
    try:
        running, snapshot = start_with_buttons(Fail=fail)
        running.click(snapshot.get_one(role="button", name="Fail"))
    finally:
        sui.set_exception_handler(previous)

    assert reported == [ValueError, RuntimeError]


def test_set_exception_handler_validates_and_returns_previous():
    def handler(error):
        pass

    previous = sui.set_exception_handler(handler)
    try:
        with pytest.raises(TypeError):
            sui.set_exception_handler(42)
        assert sui.set_exception_handler(handler) is handler
    finally:
        sui.set_exception_handler(previous)


def test_posted_tasks_and_watchers_report_exceptions(exceptions):
    running, _ = start_with_buttons()

    def failing_task():
        raise RuntimeError("posted")

    running.ui_handle().post(failing_task)
    running.drain()

    state = sui.State("a")

    def failing_watch(value):
        raise LookupError("watched")

    subscription = state.watch(failing_watch)
    state.set("b")
    subscription.unsubscribe()

    assert [type(error) for error in exceptions] == [RuntimeError, LookupError]


@pytest.mark.parametrize("exit_exception", [KeyboardInterrupt(), SystemExit(3)])
def test_exit_exceptions_propagate_from_the_driving_call(exceptions, exit_exception):
    def stop():
        raise exit_exception

    running, snapshot = start_with_buttons(Stop=stop)

    with pytest.raises(type(exit_exception)):
        running.click(snapshot.get_one(role="button", name="Stop"))
    assert exceptions == []


def test_request_exit_without_a_running_loop_returns_false():
    running, _ = start_with_buttons()
    assert running.ui_handle().request_exit() is False
