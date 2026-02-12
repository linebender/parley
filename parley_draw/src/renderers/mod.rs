// Copyright 2025 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Various renderer implementation backends.

#[cfg(any(feature = "vello_cpu", feature = "vello_hybrid"))]
pub mod vello_renderer;

#[cfg(feature = "vello_cpu")]
pub mod vello_cpu;

#[cfg(feature = "vello_hybrid")]
pub mod vello_hybrid;
