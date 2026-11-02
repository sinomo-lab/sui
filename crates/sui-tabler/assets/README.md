# Tabler Icons

The `icons/` directory contains all 5,203 outline SVGs from Tabler Icons
[v3.50.0](https://github.com/tabler/tabler-icons/tree/v3.50.0/icons/outline),
commit `6904efd245391ad396df17654738f52f6da8c40e`.
The SVG files are copied unchanged. Filled icons are not bundled.

The upstream MIT license and copyright are retained in [LICENSE](LICENSE).
SUI's Rust integration is also MIT-licensed.

`build.rs` generates the `TablerIcon` enum and native path data from these
local files; builds do not access the network. Kebab-case filenames become
PascalCase enum variants (numeric names receive an `Icon` prefix).

To update, copy the new release's `icons/outline/*.svg` and `LICENSE` here,
remove obsolete SVGs, and update this provenance and `TABLER_VERSION`.
Run the icon and widget tests and review the widget-book glyph gallery.
