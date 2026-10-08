import subprocess
import sys
from pathlib import Path

import pytest

ALLOWLIST = Path(__file__).resolve().parents[1] / "stubtest-allowlist.txt"


@pytest.mark.parametrize("module", ["sinomo_ui", "sinomo_ui._native"])
def test_stubs_match_the_runtime(module):
    pytest.importorskip("mypy")
    result = subprocess.run(
        [sys.executable, "-m", "mypy.stubtest", module, "--allowlist", str(ALLOWLIST)],
        capture_output=True,
        text=True,
        timeout=300,
    )
    assert result.returncode == 0, result.stdout + result.stderr
