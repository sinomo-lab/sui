"""Real-window event-loop tests.

Each case runs in its own interpreter: winit allows one event loop per
process. Enable with SUI_DESKTOP_TESTS=1 on a machine with a display.
"""

import subprocess
import sys
import textwrap

import pytest

pytestmark = pytest.mark.desktop

PRELUDE = """
import _thread, sys, threading
import sinomo_ui as sui

app = sui.App()
app.window(sui.Window("Desktop test").root(sui.label("Desktop test")))
"""


def run_script(body):
    return subprocess.run(
        [sys.executable, "-c", PRELUDE + textwrap.dedent(body)],
        capture_output=True,
        text=True,
        timeout=60,
    )


def test_ctrl_c_stops_run_with_keyboard_interrupt():
    result = run_script(
        """
        threading.Timer(1.0, _thread.interrupt_main).start()
        try:
            app.run()
        except KeyboardInterrupt:
            print("interrupted")
        """
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "interrupted"


def test_sys_exit_in_a_callback_leaves_run():
    result = run_script(
        """
        app.run_with_handle(lambda ui: ui.post(lambda: sys.exit(3)))
        """
    )
    assert result.returncode == 3, result.stderr


def test_request_exit_from_a_thread_returns_from_run():
    result = run_script(
        """
        def ready(ui):
            def fail():
                raise ValueError("reported, the loop keeps running")
            ui.post(fail)
            threading.Timer(1.0, ui.request_exit).start()

        app.run_with_handle(ready)
        print("returned")
        """
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "returned"
    assert "ValueError: reported, the loop keeps running" in result.stderr
