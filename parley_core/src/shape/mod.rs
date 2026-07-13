// Copyright 2026 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Text shaping utilities.

mod cache;
mod cluster;
mod data;
pub(crate) mod shaped_text;
pub(crate) mod shaper;

pub use cluster::{Char, CharCluster, SourceRange, Status, Whitespace};
pub use data::{ClusterData, ClusterInfo, to_whitespace};
