import asyncio
import threading
import time

import pytest

import sinomo_ui as sui
from sinomo_ui.aio import AsyncRunner, run_on_ui


def start_ui():
    app = sui.App()
    app.window(sui.Window("Async").root(sui.label("x")))
    running = app.start()
    return running, running.ui_handle()


def drain_until(running, condition, timeout=5.0):
    deadline = time.monotonic() + timeout
    while not condition():
        assert time.monotonic() < deadline, "timed out"
        running.drain()
        time.sleep(0.01)


def test_submit_delivers_results_on_the_ui_thread():
    running, ui = start_ui()
    ui_thread = threading.get_ident()
    results = []

    async def compute():
        await asyncio.sleep(0.01)
        assert threading.get_ident() != ui_thread
        return 42

    with AsyncRunner(ui) as runner:
        future = runner.submit(
            compute(), on_done=lambda value: results.append((value, threading.get_ident()))
        )
        assert future.result(timeout=5) == 42
        drain_until(running, lambda: results)
    assert results == [(42, ui_thread)]


def test_submit_reports_errors(exceptions):
    running, ui = start_ui()
    handled = []

    async def fail():
        raise LookupError("async failure")

    with AsyncRunner(ui) as runner:
        runner.submit(fail(), on_error=handled.append)
        runner.submit(fail())
        drain_until(running, lambda: handled and exceptions)
    assert isinstance(handled[0], LookupError)
    assert isinstance(exceptions[0], LookupError)


def test_run_on_ui_returns_the_result_to_the_coroutine():
    running, ui = start_ui()
    ui_thread = threading.get_ident()

    async def ask_ui():
        return await run_on_ui(ui, threading.get_ident)

    with AsyncRunner(ui) as runner:
        future = runner.submit(ask_ui())
        drain_until(running, future.done)
        assert future.result() == ui_thread


def test_close_cancels_pending_coroutines():
    _, ui = start_ui()
    runner = AsyncRunner(ui)
    future = runner.submit(asyncio.sleep(60))
    runner.close(timeout=5)
    assert future.cancelled()
    assert runner.loop.is_closed()
