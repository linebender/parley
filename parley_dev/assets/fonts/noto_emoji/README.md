# Noto Emoji Subset

This folder contains a small subset of [Noto Emoji](https://fonts.google.com/noto/specimen/Noto+Emoji), the monochrome version of Noto Color Emoji, licensed under the [OFL version 1.1](LICENSE.txt).
It is used to test the text presentation of emoji, and in tests whose renderer doesn't support color glyphs.

The full variable font (`NotoEmoji[wght].ttf`, version 3.002) was obtained from <https://github.com/google/fonts/tree/main/ofl/notoemoji>, and had sha256 hash `de6c18832938afc99caf132b39d6a30a19bac7f2e812e28db2535b4608d27551`.

Currently, this subset is 👀 (U+1F440), ✌ (U+270C) and the variation selectors U+FE0E and U+FE0F, at the regular weight.
It was produced using [fonttools](https://github.com/fonttools/fonttools):

```sh
fonttools varLib.instancer 'NotoEmoji[wght].ttf' wght=400 --output NotoEmoji-Regular.ttf
fonttools subset NotoEmoji-Regular.ttf \
  --output-file=NotoEmoji-Regular-subset.ttf \
  --unicodes=U+1F440,U+270C,U+FE0E,U+FE0F \
  --layout-features='*' \
  --glyph-names \
  --symbol-cmap \
  --legacy-cmap \
  --notdef-glyph --notdef-outline \
  --recommended-glyphs \
  --name-IDs='*' \
  --no-hinting
```

If in adding future tests you need more glyphs, then replacing the current subset is fine, so long as this doc is updated.
