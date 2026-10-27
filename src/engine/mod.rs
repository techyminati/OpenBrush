// OpenBrush
// Copyright © 2019 – 2026 The CipherOS Project (https://cipheros.org.in)
// Author: Aryan Sinha <techyminati>
// Licensed under the CipherOS License 2.0. See LICENSE for the full terms.

//! Image-editing engine: pixel buffers, layers, tools and file formats.
//! Nothing in here depends on the user interface.

pub mod color;
pub mod document;
pub mod raster;
pub mod history;
pub mod fill;
pub mod transform;
pub mod brush;
