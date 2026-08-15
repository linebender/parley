# Copyright 2026 the Parley Authors
# SPDX-License-Identifier: Apache-2.0 OR MIT

"""Generates font that regression tests issue 748."""

from pathlib import Path

from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen


def glyph(*contours):
    pen = TTGlyphPen(None)
    for contour in contours:
        pen.moveTo(contour[0])
        for point in contour[1:]:
            pen.lineTo(point)
        pen.closePath()
    return pen.glyph()


glyphs = {
    ".notdef": glyph(
        ((100, 0), (900, 0), (900, 800), (100, 800)),
        ((200, 100), (200, 700), (800, 700), (800, 100)),
    ),
    "space": glyph(),
    # Simplified/default: square.
    "uni4E00": glyph(((100, 0), (900, 0), (900, 800), (100, 800))),
    # Traditional/Taiwan: triangle.
    "uni4E00.traditional": glyph(((100, 0), (500, 800), (900, 0))),
    # Hong Kong: diamond.
    "uni4E00.hongkong": glyph(((100, 400), (500, 800), (900, 400), (500, 0))),
}

builder = FontBuilder(1000, isTTF=True)
builder.setupGlyphOrder(list(glyphs))
builder.setupCharacterMap({0x20: "space", 0x4E00: "uni4E00"})
builder.setupGlyf(glyphs)
builder.setupHorizontalMetrics({name: (1000, 0) for name in glyphs})
builder.setupHorizontalHeader(ascent=800, descent=-200)
builder.setupNameTable(
    {
        "familyName": "Issue 748",
        "styleName": "Regular",
        "uniqueFontIdentifier": "Issue 748 Regular",
        "fullName": "Issue 748 Regular",
        "psName": "Issue748-Regular",
    }
)
builder.setupOS2(
    sTypoAscender=800,
    sTypoDescender=-200,
    usWinAscent=800,
    usWinDescent=200,
)
builder.setupPost()
builder.setupMaxp()
builder.setupHead(created=2082844800, modified=2082844800)

addOpenTypeFeaturesFromString(
    builder.font,
    """
    languagesystem hani dflt;
    languagesystem hani ZHS;
    languagesystem hani ZHT;
    languagesystem hani ZHH;

    feature locl {
        script hani;
        language ZHT;
        sub uni4E00 by uni4E00.traditional;
        language ZHH;
        sub uni4E00 by uni4E00.hongkong;
    } locl;
    """,
)

builder.save(Path(__file__).with_name("issue748-Regular.ttf"))
