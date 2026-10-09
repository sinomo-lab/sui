from pathlib import Path

import pytest

import sinomo_ui as sui

ASSETS = Path(__file__).resolve().parents[2] / "sui-text" / "assets"


@pytest.mark.parametrize(
    "name",
    ["NotoSans-Regular.ttf", "NotoSans-Ag-subset.woff", "NotoSans-Ag-subset.woff2"],
)
def test_fonts_register_from_truetype_woff_and_woff2(name):
    app = sui.App()
    font = app.font_bytes((ASSETS / name).read_bytes())
    assert font.family == "Noto Sans"
    assert app.font_file(str(ASSETS / name)).family == "Noto Sans"


def test_invalid_font_data_is_rejected():
    app = sui.App()
    with pytest.raises(RuntimeError, match="not a TrueType"):
        app.font_bytes(bytes([0, 1, 2, 3]))
    with pytest.raises(RuntimeError, match="WOFF"):
        app.font_bytes((ASSETS / "NotoSans-Ag-subset.woff2").read_bytes()[:64])
    assert app.font_resource_count() == 0


def test_handles_made_from_an_id_have_no_family():
    assert sui.FontHandle(7).family is None
