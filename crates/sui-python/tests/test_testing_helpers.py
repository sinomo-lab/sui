import struct

import pytest

import sinomo_ui as sui


def start(root, size=(320, 200)):
    app = sui.App()
    app.window(sui.Window("Helpers", size=sui.Size(*size)).root(root))
    return app.start()


def test_queries_can_be_scoped_to_a_subtree():
    running = start(
        sui.row(
            [
                sui.semantic_region("Left", sui.button("Save")),
                sui.semantic_region("Right", sui.button("Save")),
            ]
        )
    )
    snapshot = running.render()
    assert len(snapshot.find(role="button", name="Save")) == 2
    right = snapshot.get_one(name="Right")
    save = snapshot.get_one(role="button", name="Save", within=right)
    assert save.id in [node.id for node in snapshot.find(within=right)]
    assert save.id not in [
        node.id for node in snapshot.find(within=snapshot.get_one(name="Left"))
    ]
    assert snapshot.find(within=save) == []


def test_role_queries_accept_any_spelling():
    running = start(sui.text_input("Name"))
    snapshot = running.render()
    for role in ("text_input", "text-input", "textInput"):
        assert snapshot.get_one(role=role).name == "Name"


def test_advance_time_and_settle_animations():
    running = start(sui.switch("Wi-Fi"))
    snapshot = running.render()
    assert running.frame_time == 0
    running.click(snapshot.get_one(role="switch"))
    elapsed = running.settle_animations()
    assert 0 < elapsed < 2
    assert running.frame_time >= elapsed
    assert running.settle_animations() == 0

    running.advance_time(0.5)
    assert running.frame_time == pytest.approx(elapsed + 0.5, abs=0.02)
    with pytest.raises(RuntimeError):
        running.advance_time(-1)


def test_motion_preference_off_finishes_transitions_immediately():
    sui.set_motion_preference("off")
    try:
        running = start(sui.switch("Wi-Fi"))
        running.click(running.render().get_one(role="switch"))
        assert running.settle_animations() == 0
    finally:
        sui.set_motion_preference(None)
    with pytest.raises(ValueError, match="unknown motion preference"):
        sui.set_motion_preference("slow")


def png_size(data):
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    return struct.unpack(">II", data[16:24])


def test_screenshots_render_the_window(tmp_path):
    running = start(sui.button("Capture"), size=(160, 90))
    data = running.screenshot_png()
    assert png_size(data) == (160, 90)

    path = tmp_path / "shots" / "window.png"
    running.save_screenshot(path)
    assert png_size(path.read_bytes()) == (160, 90)


def test_unsized_windows_are_captured_at_their_content_size():
    app = sui.App()
    app.window(sui.Window("No size").root(sui.button("Content")))
    width, height = png_size(app.start().screenshot_png())
    assert 0 < width < 320 and 0 < height < 200
