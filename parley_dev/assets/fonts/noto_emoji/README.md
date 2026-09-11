# Noto Emoji Subset

This folder contains a small subset of [Noto Emoji](https://fonts.google.com/noto/specimen/Noto+Emoji), licensed under the [OFL version 1.1](LICENSE.txt).
Noto Emoji is the monochrome (text presentation) counterpart to Noto Color Emoji, and this subset covers the same emoji as the Noto Color Emoji subset:

- ✅ Check Mark - \u{2705}/`:white_check_mark:`
- 👀 Eyes - \u{1f440}/`:eyes:`
- 🎉 Party Popper - \u{1f389}/`:party_popper:`
- 🤠 Face with Cowboy Hat - \u{1f920}/`:cowboy_hat_face:`
- ✌ Victory hand - \u{270c}/`:victory_hand:`

It carries no color glyph table, which makes it the text-presentation candidate in font selection tests for emoji variation sequences.
It is not part of `parley_dev::font_dirs()`, so it only participates in tests that register it explicitly.

Generated with `fonttools subset 'NotoEmoji[wght].ttf' --unicodes=U+2705,U+270C,U+1F389,U+1F440,U+1F920 --name-IDs='*'`.
