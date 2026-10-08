"""asyncio integration.

SUI runs its event loop on the main thread, and widget callbacks must stay
short. `AsyncRunner` runs an asyncio event loop on a background thread so
callbacks can start coroutines, and delivers their results back on the UI
thread::

    runner = AsyncRunner(ui)

    async def load(url):
        return await fetch(url)

    def on_press():
        runner.submit(load(url), on_done=status.set)

Coroutines reach back into the UI with `run_on_ui`, and `file_dialog` awaits a
native file dialog.
"""

from __future__ import annotations

import asyncio
import concurrent.futures
import os
import threading
from collections.abc import Callable, Coroutine, Sequence
from typing import Any, Literal, TypeVar

from ._native import UiHandle

__all__ = ["AsyncRunner", "file_dialog", "run_on_ui"]

T = TypeVar("T")


class AsyncRunner:
    """An asyncio event loop on a background thread, tied to a UI handle."""

    def __init__(self, ui: UiHandle) -> None:
        self._ui = ui
        self._loop = asyncio.new_event_loop()
        self._thread = threading.Thread(
            target=self._loop.run_forever, name="sinomo-ui-asyncio", daemon=True
        )
        self._thread.start()

    @property
    def loop(self) -> asyncio.AbstractEventLoop:
        """The background event loop."""
        return self._loop

    def submit(
        self,
        coroutine: Coroutine[Any, Any, T],
        on_done: Callable[[T], object] | None = None,
        on_error: Callable[[BaseException], object] | None = None,
    ) -> concurrent.futures.Future[T]:
        """Run `coroutine` on the background loop.

        `on_done(result)` runs on the UI thread when it finishes. If it
        raises, `on_error(exception)` runs on the UI thread instead; without
        `on_error`, the exception is reported like a callback exception. A
        cancelled coroutine calls neither.
        """
        future = asyncio.run_coroutine_threadsafe(coroutine, self._loop)

        def deliver(future: concurrent.futures.Future[T]) -> None:
            if future.cancelled():
                return
            error = future.exception()
            if error is None:
                if on_done is not None:
                    result = future.result()
                    self._ui.post(lambda: on_done(result))
            elif on_error is not None:
                self._ui.post(lambda: on_error(error))
            else:
                # Raising inside a posted task reports it through the
                # exception handler, with its traceback.
                def report() -> None:
                    raise error

                self._ui.post(report)

        future.add_done_callback(deliver)
        return future

    def close(self, timeout: float | None = None) -> None:
        """Stop the loop after cancelling pending coroutines."""

        async def shutdown() -> None:
            tasks = [
                task
                for task in asyncio.all_tasks(self._loop)
                if task is not asyncio.current_task(self._loop)
            ]
            for task in tasks:
                task.cancel()
            await asyncio.gather(*tasks, return_exceptions=True)
            self._loop.stop()

        if self._loop.is_closed():
            return
        asyncio.run_coroutine_threadsafe(shutdown(), self._loop)
        self._thread.join(timeout)
        if not self._thread.is_alive():
            self._loop.close()

    def __enter__(self) -> AsyncRunner:
        return self

    def __exit__(self, *_: object) -> None:
        self.close()


async def run_on_ui(ui: UiHandle, function: Callable[[], T]) -> T:
    """Run `function` on the UI thread and return its result to the caller's
    event loop."""
    loop = asyncio.get_running_loop()
    future: asyncio.Future[T] = loop.create_future()

    def call() -> None:
        try:
            result = function()
        except BaseException as error:  # delivered to the awaiting coroutine
            loop.call_soon_threadsafe(_set_exception, future, error)
        else:
            loop.call_soon_threadsafe(_set_result, future, result)

    ui.post(call)
    return await future


async def file_dialog(
    ui: UiHandle,
    mode: Literal["open", "open-multiple", "save", "folder", "folders"] = "open",
    title: str | None = None,
    filters: Sequence[tuple[str, Sequence[str]]] | None = None,
    directory: str | os.PathLike[str] | None = None,
    name: str | None = None,
) -> list[str] | None:
    """Show a native file dialog and return the chosen paths, or `None` when
    the user cancels."""
    loop = asyncio.get_running_loop()
    future: asyncio.Future[list[str] | None] = loop.create_future()
    ui.show_file_dialog(
        lambda paths: loop.call_soon_threadsafe(_set_result, future, paths),
        mode=mode,
        title=title,
        filters=filters,
        directory=directory,
        name=name,
    )
    return await future


def _set_result(future: asyncio.Future[T], result: T) -> None:
    if not future.done():
        future.set_result(result)


def _set_exception(future: asyncio.Future[Any], error: BaseException) -> None:
    if not future.done():
        future.set_exception(error)
