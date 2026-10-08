import subprocess
import sys
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


@pytest.mark.parametrize("example", sorted(path.name for path in EXAMPLES.glob("*.py")))
def test_example_runs(example):
    result = subprocess.run(
        [sys.executable, str(EXAMPLES / example)],
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert result.returncode == 0, result.stderr
