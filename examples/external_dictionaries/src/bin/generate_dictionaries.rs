// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Exports ICU4X's built-in dictionaries to be loaded by the main binary of this example.

use std::error::Error;

use icu_provider::dynutil::UpcastDataPayload;
use icu_provider::export::{DataExporter, ExportMarker, FlushMetadata};
use icu_provider::{
    DataError, DataIdentifierBorrowed, DataMarker, DataMarkerAttributes, DataProvider, DataRequest,
};
use icu_provider_blob::export::BlobExporter;
use icu_segmenter::provider::{Baked, SegmenterDictionaryAutoV1, SegmenterDictionaryExtendedV1};

fn main() -> Result<(), Box<dyn Error>> {
    let output_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../_output");
    std::fs::create_dir_all(&output_dir)?;
    let path = output_dir.join("dictionaries.postcard");
    let mut bytes = Vec::new();
    {
        let mut exporter = BlobExporter::new_with_sink(Box::new(&mut bytes));
        // ICU4X 2.3 dictionary identifiers; revisit these when upgrading ICU.
        export::<SegmenterDictionaryAutoV1>(&exporter, &["cjdict"])?;
        export::<SegmenterDictionaryExtendedV1>(
            &exporter,
            &["burmesedict", "khmerdict", "laodict", "thaidict"],
        )?;
        exporter.close()?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

fn export<M: DataMarker>(exporter: &BlobExporter<'_>, ids: &[&str]) -> Result<(), DataError>
where
    Baked: DataProvider<M>,
    ExportMarker: UpcastDataPayload<M>,
{
    for id in ids {
        let request = DataRequest {
            id: DataIdentifierBorrowed::for_marker_attributes(
                DataMarkerAttributes::from_str_or_panic(id),
            ),
            ..DataRequest::default()
        };
        let response = DataProvider::<M>::load(&Baked, request)?;
        exporter.put_payload(M::INFO, request.id, &ExportMarker::upcast(response.payload))?;
    }
    exporter.flush(M::INFO, FlushMetadata::default())
}
