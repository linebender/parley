# External segmentation

Load binary
See the docs on `DictionaryProvider` for more details.

This consists of two binaries - one generates the dictionary, then one loads it for layout.
Run with:

```sh
cargo run -p external_dictionaries --features generate --bin generate_dictionaries
cargo run -p external_dictionaries
```

Note that the generated file will be under icu4x's license.
