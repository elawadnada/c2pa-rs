// Copyright 2024 Adobe. All rights reserved.
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

use async_generic::async_generic;

use crate::{
    claim::ClaimAssetData, error::Result, manifest_store::ManifestStore,
    status_tracker::DetailedStatusTracker, store::Store, validation_status::ValidationStatus,
    CAIRead, CAIReadWrite, Manifest,
};

/// A reader for the manifest store
pub struct Reader {
    pub(crate) manifest_store: ManifestStore,
}

impl Reader {
    pub fn new() -> Self {
        Self {
            manifest_store: ManifestStore::new(),
        }
    }

    /// Create a manifest store Reader from a stream
    /// # Arguments
    /// * `format` - The format of the stream
    /// * `stream` - The stream to read from
    /// # Returns
    /// A reader for the manifest store
    /// # Errors
    /// If the stream is not a valid manifest store
    #[async_generic(async_signature(
        format: &str,
        stream: &mut dyn CAIRead,
    ))]
    pub fn from_stream(format: &str, stream: &mut dyn CAIRead) -> Result<Reader> {
        let verify = true; // todo: get this from config
        let reader = if _sync {
            ManifestStore::from_stream(format, stream, verify)
        } else {
            ManifestStore::from_stream_async(format, stream, verify).await
        }?;
        Ok(Reader {
            manifest_store: reader,
        })
    }

    /// Create a manifest store Reader from existing c2pa_data and a stream
    /// You can use this to validate a remote manifest or a sidecar manifest
    /// # Arguments
    /// * `c2pa_data` - The c2pa data (a manifest store in JUMBF format)
    /// * `format` - The format of the stream
    /// * `stream` - The stream to verify the store against
    /// # Returns
    /// A reader for the manifest store
    /// # Errors
    /// If the c2pa_data is not valid, or severe errors occur in validation
    /// validation status should be checked for non severe errors
    #[async_generic(async_signature(
        c2pa_data: &[u8],
        format: &str,
        stream: &mut dyn CAIRead,
    ))]
    pub fn from_c2pa_data_and_stream(
        c2pa_data: &[u8],
        format: &str,
        stream: &mut dyn CAIRead,
    ) -> Result<Reader> {
        let mut validation_log = DetailedStatusTracker::new();

        // first we convert the JUMBF into a usable store
        let store = Store::from_jumbf(c2pa_data, &mut validation_log)?;

        if _sync {
            Store::verify_store(
                &store,
                &mut ClaimAssetData::Stream(stream, format),
                &mut validation_log,
            )?;
        } else {
            Store::verify_store_async(
                &store,
                &mut ClaimAssetData::Stream(stream, format),
                &mut validation_log,
            )
            .await?;
        }

        Ok(Reader {
            manifest_store: ManifestStore::from_store(&store, &validation_log),
        })
    }

    /// Get the manifest store as a JSON string
    pub fn json(&self) -> String {
        self.manifest_store.to_string()
    }

    /// Get the validation status of the manifest store
    pub fn status(&self) -> Option<&[ValidationStatus]> {
        self.manifest_store.validation_status()
    }

    /// Get the active manifest if there is one
    pub fn active_manifest(&self) -> Option<&Manifest> {
        self.manifest_store.get_active()
    }

    /// Write a resource identified by uri to the given stream
    /// # Arguments
    /// * `uri` - The URI of the resource to write
    /// * `stream` - The stream to write to
    /// # Returns
    /// The number of bytes written
    /// # Errors
    /// If the resource does not exist
    pub fn resource(&self, uri: &str, stream: &mut dyn CAIReadWrite) -> Result<usize> {
        self.manifest_store
            .get_resource(uri, stream)
            .map(|size| size as usize)
    }
}

impl Default for Reader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.json().as_str())
    }
}
