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


def run_script(body, *args):
    return subprocess.run(
        [sys.executable, "-c", PRELUDE + textwrap.dedent(body), *args],
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


@pytest.mark.skipif(sys.platform != "win32", reason="sends a native Windows close request")
@pytest.mark.parametrize("close_delay", [0, 1])
@pytest.mark.parametrize("window_count", [1, 2])
def test_unhandled_native_close_returns_from_counter_run(close_delay, window_count):
    result = run_script(
        """
        import ctypes
        from ctypes import wintypes
        import os
        import time

        user32 = ctypes.WinDLL("user32", use_last_error=True)
        user32.FindWindowW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
        user32.FindWindowW.restype = wintypes.HWND
        user32.GetWindowThreadProcessId.argtypes = [
            wintypes.HWND, ctypes.POINTER(wintypes.DWORD)
        ]
        user32.GetWindowThreadProcessId.restype = wintypes.DWORD
        user32.PostMessageW.argtypes = [
            wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM
        ]
        user32.PostMessageW.restype = wintypes.BOOL

        close_delay = float(sys.argv[1])
        titles = [f"Counter close test {os.getpid()}-{i}" for i in range(int(sys.argv[2]))]
        count = sui.State(0)
        app = sui.App()
        for title in titles:
            app.window(
                sui.Window(title, size=sui.Size(320, 160)).root(
                    sui.column(
                        [
                            sui.label(count),
                            sui.button("Add one", on_press=lambda: count.set(count.get() + 1)),
                        ],
                        gap=8,
                        align_items="center",
                    )
                )
            )

        def find_window(title):
            deadline = time.monotonic() + 15
            while time.monotonic() < deadline:
                hwnd = user32.FindWindowW(None, title)
                if hwnd:
                    owner = wintypes.DWORD()
                    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
                    assert owner.value == os.getpid()
                    return hwnd
                time.sleep(0.05)
            raise AssertionError("counter window was not created")

        closed = []

        def close_windows():
            for title in titles:
                hwnd = find_window(title)
                time.sleep(close_delay)
                assert user32.PostMessageW(hwnd, 0x0010, 0, 0)  # WM_CLOSE
                deadline = time.monotonic() + 15
                while user32.FindWindowW(None, title):
                    assert time.monotonic() < deadline, "counter window did not close"
                    time.sleep(0.05)
                closed.append(title)

        closer = threading.Thread(target=close_windows, daemon=True)
        closer.start()
        app.run()
        # Check before joining: exiting the process must not be what destroys
        # the windows, and the loop must keep running for any remaining window.
        assert not any(user32.FindWindowW(None, title) for title in titles)
        closer.join(timeout=5)
        assert closed == titles
        print("closed")
        """,
        str(close_delay),
        str(window_count),
    )
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "closed"
