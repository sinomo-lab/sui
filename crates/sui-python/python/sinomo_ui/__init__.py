"""Python bindings for SUI, a retained-mode UI toolkit rendered with wgpu.

Import the package under a short alias::

    import sinomo_ui as sui

    app = sui.App()
    app.window(sui.Window("Hello").root(sui.label("Ready")))
    app.run()
"""

from ._native import *  # noqa: F403
from ._native import __version__
