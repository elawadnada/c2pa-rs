// Copyright 2022 Adobe. All rights reserved.
// This file is licensed to you under the Apache License,
// Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
// or the MIT license (http://opensource.org/licenses/MIT),
// at your option.

// Unless required by applicable law or agreed to in writing,
// this software is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or
// implied. See the LICENSE-MIT and LICENSE-APACHE files for the
// specific language governing permissions and limitations under
// each license.

use std::{
    fmt,
    io::{Read, Seek, Write},
    path::Path,
};

use tempfile::NamedTempFile;

use crate::error::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashBlockObjectType {
    Cai,
    Xmp,
    Other,
}

impl fmt::Display for HashBlockObjectType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
#[derive(Debug)]
pub struct HashObjectPositions {
    pub offset: usize, // offset from beginning of file to the beginning of object
    pub length: usize, // length of object
    pub htype: HashBlockObjectType, // type of hash block object
}
/// CAIReader trait to insure CAILoader method support both Read & Seek
pub trait CAIRead: Read + Seek {}

impl CAIRead for std::fs::File {}
impl CAIRead for std::io::Cursor<&[u8]> {}
impl CAIRead for std::io::Cursor<&mut [u8]> {}
impl CAIRead for std::io::Cursor<Vec<u8>> {}
impl CAIRead for NamedTempFile {}

pub trait CAIReadWrite: CAIRead + Write {}

impl CAIReadWrite for std::fs::File {}
impl CAIReadWrite for std::io::Cursor<&mut [u8]> {}
impl CAIReadWrite for std::io::Cursor<Vec<u8>> {}
impl CAIReadWrite for NamedTempFile {}

// Interface for in memory CAI reading
pub trait CAIReader: Sync + Send {
    // Return entire CAI block as Vec<u8>
    fn read_cai(&self, asset_reader: &mut dyn CAIRead) -> Result<Vec<u8>>;

    // Get XMP block
    fn read_xmp(&self, asset_reader: &mut dyn CAIRead) -> Option<String>;
}

pub trait CAIWriter: Sync + Send {
    // Writes store_bytes into output_steam using input_stream as the source asset
    fn write_cai(
        &self,
        input_stream: &mut dyn CAIRead,
        output_stream: &mut dyn CAIReadWrite,
        store_bytes: &[u8],
    ) -> Result<()>;

    // Finds location where the C2PA manifests will be placed in the asset specified by input_stream
    fn get_object_locations_from_stream(
        &self,
        input_stream: &mut dyn CAIRead,
    ) -> Result<Vec<HashObjectPositions>>;

    // Remove entire C2PA manifest store from asset
    fn remove_cai_store_from_stream(
        &self,
        input_stream: &mut dyn CAIRead,
        output_stream: &mut dyn CAIReadWrite,
    ) -> Result<()>;
}

pub trait AssetIO: Sync + Send {
    // Create instance of AssetIO handler.  The extension type is passed in so
    // that format specific customizations can be used during manifest embedding
    fn new(asset_type: &str) -> Self
    where
        Self: Sized;

    // Return AssetIO handler for this asset type
    fn get_handler(&self, asset_type: &str) -> Box<dyn AssetIO>;

    // Return streaming reader for this asset type
    fn get_reader(&self) -> &dyn CAIReader;

    // Return streaming writer if available
    fn get_writer(&self, _asset_type: &str) -> Option<Box<dyn CAIWriter>> {
        None
    }

    // Return entire CAI block as Vec<u8>
    fn read_cai_store(&self, asset_path: &Path) -> Result<Vec<u8>>;

    // Write the CAI block to an asset
    fn save_cai_store(&self, asset_path: &Path, store_bytes: &[u8]) -> Result<()>;

    /// List of standard object offsets
    /// If the offsets exist return the start of those locations other it should
    /// return the calculated location of when it should start.  There may still be a
    /// length if the format contains extra header information for example.
    fn get_object_locations(&self, asset_path: &Path) -> Result<Vec<HashObjectPositions>>;

    // Remove entire C2PA manifest store from asset
    fn remove_cai_store(&self, asset_path: &Path) -> Result<()>;

    // List of supported extensions and mime types
    fn supported_types(&self) -> &[&str];

    /// OPTIONAL INTERFACES

    // Returns [`AssetPatch`] trait if this I/O handler supports patching.
    fn asset_patch_ref(&self) -> Option<&dyn AssetPatch> {
        None
    }

    // Returns [`RemoteRefEmbed`] trait if this I/O handler supports remote reference embedding.
    fn remote_ref_writer_ref(&self) -> Option<&dyn RemoteRefEmbed> {
        None
    }
}

// `AssetPatch` optimizes output generation for asset_io handlers that
// are able to patch blocks of data without changing any other data. The
// resultant file must still be a valid asset. This saves having to rewrite
// assets since only the patched bytes are modified.
pub trait AssetPatch {
    // Patches an existing manifest store with new manifest store.
    // Only existing manifest stores of the same size may be patched
    // since any other changes will invalidate asset hashes.
    fn patch_cai_store(&self, asset_path: &Path, store_bytes: &[u8]) -> Result<()>;
}

// Type of remote reference to embed.  Some of the listed
// emums are for future uses and experiments.
#[allow(unused_variables)]
pub enum RemoteRefEmbedType {
    Xmp(String),
    StegoS(String),
    StegoB(Vec<u8>),
    Watermark(String),
}

// `RemoteRefEmbed` is used to embed remote references to external manifests.  The
// technique used to embed a reference varies bases on the type of embedding.  Not
// all embedding choices need be supported.
pub trait RemoteRefEmbed {
    // Embed RemoteRefEmbedType into the asset
    fn embed_reference(&self, asset_path: &Path, embed_ref: RemoteRefEmbedType) -> Result<()>;
    // Embed RemoteRefEmbedType into the asset stream
    fn embed_reference_to_stream(
        &self,
        source_stream: &mut dyn CAIRead,
        output_stream: &mut dyn CAIReadWrite,
        embed_ref: RemoteRefEmbedType,
    ) -> Result<()>;
}
