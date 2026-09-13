# Portable text fonts

On WebAssembly and Android, `sui-text` registers these embedded font bytes with
its text engine before shaping any text:

| Asset | Purpose |
| --- | --- |
| `NotoSans-Regular.ttf` | Generic sans-serif |
| `NotoSerif-Variable.ttf` | Generic serif |
| `NotoSansMono-Variable.ttf` | Generic monospace |
| `NotoSansArabic-Regular.ttf` | Arabic-script fallback, including joining forms and marks |
| `NotoSansHebrew-Regular.ttf` | Hebrew fallback, including vowel points and cantillation |

Desktop builds discover installed system fonts. Applications can also register
their own font bytes through SUI's resource registry on any platform.

The existing generic font assets retain their original provenance:

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

Keep each corresponding SIL Open Font License 1.1 file with redistributed font
assets.

The Arabic and Hebrew assets are the complete regular-weight TTFs from the
upstream releases below, copied without modification or character subsetting.
They add 337,808 bytes of font data and require no network requests. The adjacent
`NotoSansArabic-LICENSE` and `NotoSansHebrew-LICENSE` files contain their upstream
SIL Open Font License 1.1 notices.

| Asset | Upstream release | File within release archive |
| --- | --- | --- |
| `NotoSansArabic-Regular.ttf` | [Noto Sans Arabic 2.013](https://github.com/notofonts/arabic/releases/tag/NotoSansArabic-v2.013) | `NotoSansArabic/full/ttf/NotoSansArabic-Regular.ttf` |
| `NotoSansHebrew-Regular.ttf` | [Noto Sans Hebrew 3.001](https://github.com/notofonts/hebrew/releases/tag/NotoSansHebrew-v3.001) | `NotoSansHebrew/full/ttf/NotoSansHebrew-Regular.ttf` |

SHA-256 checksums:

```text
7ed3fe069312aceac454f17cf613a30f95271d6ed7ce58005ed4d016bd3823d7  NotoSansArabic-Regular.ttf
671951828bd5c95db818e5bb12dcea2d0c0dda00311888522be061ee6835125e  NotoSansHebrew-Regular.ttf
```

When updating either asset, copy its full upstream font and license, update this
provenance, and run the `portable_fonts` tests. These tests use only embedded
fonts to check script coverage, contextual Arabic forms, combining marks,
bidirectional layout, and caret hit testing.

The web demo additionally ships full CJK and color emoji fonts as independently
cacheable downloads, with small embedded subsets for its built-in samples.
Those larger assets stay outside WASM; see the [demo font documentation](../../sui-demo/assets/README.md).
