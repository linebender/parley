# Noto Sans CJK JP Subset

This folder contains a subset of [Noto Sans CJK JP](https://github.com/notofonts/noto-cjk/blob/f8d157532fbfaeda587e826d4cd5b21a49186f7c/Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf), licensed under the [OFL version 1.1](LICENSE.txt).
We do not include the full font, because the complete CJK glyph set would increase the repository size too much.

The subset covers these Unicode ranges, plus the kanji 年日本至語:

- `U+0020–U+007E` — Basic Latin (ASCII)
- `U+3000–U+303F` — CJK Symbols and Punctuation
- `U+3040–U+30FF` — Hiragana and Katakana
- `U+FF00–U+FFEF` — Halfwidth and Fullwidth Forms
- `U+4E00–U+4FFF` — a slice of CJK Unified Ideographs

The subset retains the `vert`/`vrt2` features and the `vhea`/`vmtx`/`VORG`/`BASE` tables, so it is suitable for testing vertical writing.
