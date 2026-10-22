import runpy
from importlib import metadata
from pathlib import Path

import pytest

import sinomo_ui as sui

EXAMPLES = Path(__file__).resolve().parents[1] / "examples"


def test_version_matches_distribution_metadata():
    assert sui.__version__ == metadata.version("sinomo-ui")


def test_widget_factories_are_snake_case_only():
    assert callable(sui.button)
    assert sui.button.__name__ == "button"
    assert not hasattr(sui, "Button")
    assert not hasattr(sui, "VirtualScrollView")


def test_classes_report_the_public_module():
    assert sui.App.__module__ == "sinomo_ui"
    assert sui.State.__module__ == "sinomo_ui"


def start_example(name):
    namespace = runpy.run_path(str(EXAMPLES / name))
    return namespace, namespace["app"].start()


def test_counter_example_counts_disables_and_changes_theme():
    namespace, running = start_example("counter.py")
    running.click(running.render().get_one(role="button", name="Increment"))
    assert namespace["count"].get() == 1

    running.click(running.render().get_one(role="checkbox", name="Enabled"))
    button = running.render().get_one(role="button", name="Increment")
    assert button.disabled
    with pytest.raises(RuntimeError, match="not actionable"):
        running.click(button)
    assert namespace["count"].get() == 1

    theme = namespace["theme"]
    dark_background = theme.color("window")
    running.click(running.render().get_one(role="switch", name="Dark theme"))
    running.drain()
    assert theme.color("window") != dark_background


def test_custom_widget_example_repaints_after_a_click():
    namespace, running = start_example("custom_widget.py")
    snapshot = running.render()
    assert snapshot.draw_image_count == 1
    meter = snapshot.get_one(role="progress_bar", name="Level meter")
    running.click(meter)
    assert namespace["meter"].value == pytest.approx(0.5)
    updated = running.render()
    assert updated.semantics_values != snapshot.semantics_values
    assert updated.draw_image_count == 1


def test_external_surface_example_uploads_the_texture():
    _, running = start_example("external_surface.py")
    snapshot = running.render()
    assert snapshot.draw_image_count == 1
    assert snapshot.registered_image_count == 1
    assert snapshot.get_one(name="CPU upload preview").bounds.width > 0
