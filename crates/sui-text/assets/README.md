# Bundled font assets

These fonts are embedded only in `wasm32` builds so SUI's generic theme
families resolve without relying on browser or host-installed fonts.

- `NotoSans-Regular.ttf` is the existing sans-serif fallback and is covered by
  `NotoSans-LICENSE`.
- `NotoSerif-Variable.ttf` is the unmodified
  `ofl/notoserif/NotoSerif[wdth,wght].ttf` artifact from
  `google/fonts@ec0464b978de222073645d6d3366f3fdf03376d8`. Its SHA-256 is
  `4d8e6761424656867019081a1a01336f3cb086982682698714054fc33f782713` and its
  SIL Open Font License is in `NotoSerif-LICENSE`.
- `NotoSansMono-Variable.ttf` is the unmodified
  `ofl/notosansmono/NotoSansMono[wdth,wght].ttf` artifact from the same pinned
  Google Fonts revision. Its SHA-256 is
  `2cb2adb378a8f574213e23df697050b83c54c27df465a2015552740b2769a081` and its
  SIL Open Font License is in `NotoSansMono-LICENSE`.

The upstream artifacts are distributed by Google Fonts under the SIL Open
Font License 1.1. Keep each corresponding license file with redistributed font
assets.
