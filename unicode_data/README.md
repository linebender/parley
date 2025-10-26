# Unicode Data

This crate exports Unicode data required by the Parley text pipeline.

Principally, it supports building a custom data structure from ICU4X data providers to allow for a single lookup on a character to identify all pertinent information for text analysis.

Separately, it supports the building of data providers for word and line segmentation (and the like).

The crate supports the `build` feature flag which is used to access functionality that enables exporting ICU data for consumption into Parley.
