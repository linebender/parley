// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use icu_provider::{
    DataError, DataErrorKind, DataIdentifierBorrowed, DataMarker, DataMarkerAttributes,
    DataProvider, DataRequest, DataResponse, ResultDataError,
};
use icu_segmenter::provider::{SegmenterDictionaryAutoV1, SegmenterDictionaryExtendedV1};
use parley::DictionaryProvider;

/// An implementation of [`DictionaryProvider`] which builds a cache around a different `DictionaryProvider`.
pub(super) struct CachingDictionary {
    auto: CachedProvider<SegmenterDictionaryAutoV1>,
    extended: CachedProvider<SegmenterDictionaryExtendedV1>,
}

impl CachingDictionary {
    /// Loads each dictionary `icu_segmenter` v2.3.0 uses.
    /// Errors (except for missing identifiers) are returned.
    pub(super) fn try_new<P: DictionaryProvider + ?Sized>(provider: &P) -> Result<Self, DataError> {
        // ICU4X's invariant dictionary identifiers. Revisit when upgrading ICU.
        Ok(Self {
            auto: CachedProvider::load(provider, &["cjdict"])?,
            extended: CachedProvider::load(
                provider,
                &["burmesedict", "khmerdict", "laodict", "thaidict"],
            )?,
        })
    }
}

struct CachedProvider<M: DataMarker> {
    entries: Vec<(&'static str, DataResponse<M>)>,
}

impl<M: DataMarker> CachedProvider<M> {
    fn load<P: DataProvider<M> + ?Sized>(
        provider: &P,
        ids: &[&'static str],
    ) -> Result<Self, DataError> {
        let mut entries = Vec::new();
        for &id in ids {
            let mut request = DataRequest {
                id: DataIdentifierBorrowed::for_marker_attributes(
                    DataMarkerAttributes::from_str_or_panic(id),
                ),
                ..DataRequest::default()
            };
            request.metadata.silent = true;
            request.metadata.attributes_prefix_match = true;
            if let Some(response) = provider.load(request).allow_identifier_not_found()? {
                entries.push((id, response));
            }
        }
        Ok(Self { entries })
    }

    fn get(&self, req: DataRequest<'_>) -> Result<DataResponse<M>, DataError>
    where
        icu_provider::DataPayload<M>: Clone,
    {
        if req.id.locale.is_unknown()
            && let Some((_, response)) = self
                .entries
                .iter()
                .find(|(id, _)| *id == req.id.marker_attributes.as_str())
        {
            return Ok(DataResponse {
                metadata: response.metadata.clone(),
                payload: response.payload.clone(),
            });
        }
        Err(DataErrorKind::IdentifierNotFound.with_req(M::INFO, req))
    }
}

impl DataProvider<SegmenterDictionaryAutoV1> for CachingDictionary {
    fn load(
        &self,
        req: DataRequest<'_>,
    ) -> Result<DataResponse<SegmenterDictionaryAutoV1>, DataError> {
        self.auto.get(req)
    }
}

impl DataProvider<SegmenterDictionaryExtendedV1> for CachingDictionary {
    fn load(
        &self,
        req: DataRequest<'_>,
    ) -> Result<DataResponse<SegmenterDictionaryExtendedV1>, DataError> {
        self.extended.get(req)
    }
}
