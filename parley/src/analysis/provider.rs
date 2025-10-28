#![allow(unsafe_code, reason = "ICU4X uses fast bytearray loading in its baked data sources.")]
#![allow(elided_lifetimes_in_paths)]
#![allow(unreachable_pub)]

include!(concat!(env!("OUT_DIR"), "/baked_data/mod.rs"));
include!(concat!(env!("OUT_DIR"), "/baked_data/composite_blob.rs"));

pub struct BakedProvider;
impl_data_provider!(BakedProvider);

pub(crate) static PROVIDER: BakedProvider = BakedProvider;