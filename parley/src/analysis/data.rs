// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(
    unsafe_code,
    reason = "ICU4X uses fast bytearray loading in its baked data sources."
)]
#![allow(elided_lifetimes_in_paths)]
#![allow(unreachable_pub)]
#![allow(clippy::unseparated_literal_suffix)]
#![allow(unused_unsafe)]

include!("../../data/icu4x_data/mod.rs");

include!("../../data/composite/mod.rs");

pub struct Provider;
impl_data_provider!(Provider);
