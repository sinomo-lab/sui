# Web demo fonts

The browser demo embeds small subsets of the CJK and color emoji fallback fonts
for its built-in samples. Trunk copies the full upstream fonts alongside the
site, and the WebAssembly startup code fetches and registers them with SUI's
text engine before running the application. Keeping the full fonts outside the
WebAssembly module makes them independently cacheable while preserving
arbitrary CJK and emoji user input.

CSS `@font-face` alone is not enough: SUI shapes and renders text through its
own WGPU text stack instead of the browser's DOM text renderer, so it must
receive the font bytes explicitly. If an external font request fails, the
embedded subsets still keep the built-in demo samples renderable.

All fonts are covered by the adjacent Noto license files.

The subsets cover the non-Latin samples exercised by the demo:

- `NotoSansCJKsc-DemoSubset.otf`: `你好日本語한국어中文候補を`
- `NotoColorEmoji-DemoSubset.ttf`: `🙂`, `✅`, `🎨`, and emoji variation
  selector U+FE0F

Regenerate the files from the repository root with FontTools:

```bash
uvx --from 'fonttools==4.63.0' pyftsubset \
  crates/sui-demo/assets/NotoSansCJKsc-Regular.otf \
  --text='你好日本語한국어中文候補を' \
  --output-file=crates/sui-demo/assets/NotoSansCJKsc-DemoSubset.otf \
  --layout-features='*'

uvx --from 'fonttools==4.63.0' pyftsubset \
  crates/sui-demo/assets/NotoColorEmoji.ttf \
  --unicodes='U+1F642,U+2705,U+1F3A8,U+FE0F' \
  --output-file=crates/sui-demo/assets/NotoColorEmoji-DemoSubset.ttf \
  --layout-features='*'
```

When adding new CJK or emoji samples to the web demo, extend the corresponding
subset input and regenerate the checked-in font before publishing the site.
