import os

import pytest

import sinomo_ui as sui


def pytest_collection_modifyitems(config, items):
    if os.environ.get("SUI_DESKTOP_TESTS") == "1":
        return
    skip = pytest.mark.skip(reason="opens desktop windows; set SUI_DESKTOP_TESTS=1")
    for item in items:
        if "desktop" in item.keywords:
            item.add_marker(skip)


@pytest.fixture
def exceptions():
    """Collect callback exceptions instead of printing them."""
    seen: list[BaseException] = []
    previous = sui.set_exception_handler(seen.append)
    yield seen
    sui.set_exception_handler(previous)
