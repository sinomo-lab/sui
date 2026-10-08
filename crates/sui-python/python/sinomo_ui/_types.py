"""Type aliases and structural types used by the ``sinomo_ui`` API.

These are ordinary runtime objects rather than stub-only names, so
``sinomo_ui.Axis`` and friends can be imported at runtime as well as by type
checkers.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Literal, Optional, Protocol, TypeAlias, TypedDict, Union

from ._native import Constraints, Event, EventContext, Paint, Rect, Semantics, Size

__all__ = [
    "Axis",
    "BindingValue",
    "FontStretch",
    "FontStyle",
    "IconGlyph",
    "ImageFit",
    "RichAttachmentOptions",
    "RichExtensionOptions",
    "ScrollAxes",
    "SemanticTone",
    "SurfaceBorder",
    "SurfaceElevation",
    "SurfaceRole",
    "WidgetCallbacks",
]

BindingValue: TypeAlias = Union[str, int, float, bool]
"""A value that can be stored in a ``State`` or sent with a message."""

# Spelled as unions of single literals: a type checker reads
# `Literal["a", "b"]` as such a union, and stubtest requires the runtime
# alias to be a `Union` to match.
Axis: TypeAlias = Union[Literal["horizontal"], Literal["vertical"]]
ScrollAxes: TypeAlias = Union[Axis, Literal["both"]]
SurfaceRole: TypeAlias = str
SurfaceBorder: TypeAlias = str
SurfaceElevation: TypeAlias = str
SemanticTone: TypeAlias = str
IconGlyph: TypeAlias = str
ImageFit: TypeAlias = str
FontStyle: TypeAlias = str
FontStretch: TypeAlias = str


class RichAttachmentOptions(TypedDict, total=False):
    media_type: str
    source: str
    size_bytes: int
    description: str


class RichExtensionOptions(TypedDict, total=False):
    summary: str
    body: str
    status: str
    initially_expanded: bool
    metadata: Mapping[str, str]


class WidgetCallbacks(Protocol):
    """The full set of callbacks a custom ``Widget`` object may provide.

    Every callback is optional: ``Widget`` accepts any object and only calls
    the methods it defines, so most objects implement just a subset of this
    protocol.
    """

    name: str

    def measure(self, constraints: Constraints) -> Size: ...
    def measure_with_children(
        self, constraints: Constraints, child_sizes: Sequence[Size]
    ) -> Size: ...
    def arrange(self, bounds: Rect, child_sizes: Sequence[Size]) -> Sequence[Rect]: ...
    def event(self, event: Event) -> Optional[bool]: ...
    def event_with_context(self, event: Event, context: EventContext) -> Optional[bool]: ...
    def paint(self, paint: Paint) -> None: ...
    def semantics(self, semantics: Semantics) -> None: ...
