This is a nascent project for implementing a rich text layout engine backed
by the [swash](https://github.com/dfrg/swash) crate and implementing the 
[piet](https://github.com/linebender/piet) text API.


## Code structure

A guide to parley's source code.


```
src
|
| MISC
|
|-- lib.rs  - Entry point. Contains only re-exports.
|-- util.rs - Float comparision helpers
| 
|-- context.rs - Contains LayoutContext and RangedBuilder which collectively make up the top-level API in Parley
|
|
| STYLE & STYLE RESOLUTION
|
|-- style - Types representing styles that can be set on a range of text
|   |
|   |-- mod.rs   - `StyleProperty` enum representing all of the styles that can be set on a range of text
|   |-- font.rs  - Types to represent font styles (e.g. font family) in the public API
|   |-- brush.rs - `Brush` trait that represents the color of glyphs or decorations.
|
|-- resolve - Resolving styles from the end-user API into styles that parley can use for shaping, layout, etc.
|   |
|   |-- mod.rs   - Types to represent font styles (e.g. font family) in the public API
|   |-- range.rs - Types that allow overlapping ranges of single style properties to be resolved into non-overlapping ranges of all style properties
|   |-- tree.rs  - (FILE EMPTY) Placeholder for an alternative to range.rs that works on trees of styles instead of ranges.
|
| BIDI, SHAPING and LAYOUT
| 
|-- bidi.rs  - Splits a string of text into a sequence of runs according to the Unicode Bidirectional Algorithm
|-- shape.rs - Takes output of bidi and style systems and splits text into runs for shaping and then shapes those runs.
|
|-- layout - Line breaking, layout and alignment of shaped text runs
|   |
|   |-- line
|   |  |
|   |  |-- greedy.rs - The actual line breaking implementation
|   |  |-- mod.rs    - Supporting code for line breaking
|   |
|   |-- mod.rs       - Data structures for line breaking
|   |-- data.rs      - Data structures for line breaking. The push_run method used in shaping.
|   |-- run.rs       - Method implementation for Run type
|   |-- cluster.rs   - Method implementations for the Cluster type
|   |-- cursor.rs    - Cursor type representing a position within a layout. Hit testing.
```
