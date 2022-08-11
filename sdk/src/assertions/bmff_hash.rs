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
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

use crate::{
    assertion::{Assertion, AssertionBase, AssertionCbor},
    assertions::labels,
    asset_handlers::bmff_io::bmff_to_jumbf_exclusions,
    cbor_types::UriT,
    error::Result,
    utils::hash_utils::{hash_asset_by_alg, verify_asset_by_alg, verify_by_alg},
    Error,
};

const ASSERTION_CREATION_VERSION: usize = 1;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct ExclusionsMap {
    pub xpath: String,
    pub length: Option<u32>,
    pub data: Option<Vec<DataMap>>,
    pub subset: Option<Vec<SubsetMap>>,
    pub version: Option<u8>,
    pub flags: Option<ByteBuf>,
    pub exact: Option<bool>,
}

impl ExclusionsMap {
    pub fn new(xpath: String) -> Self {
        ExclusionsMap {
            xpath,
            length: None,
            data: None,
            subset: None,
            version: None,
            flags: None,
            exact: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct MerkleMap {
    #[serde(rename = "uniqueId")]
    pub unique_id: u32,

    #[serde(rename = "localId")]
    pub local_id: u32,

    pub count: u32,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub alg: Option<String>,

    #[serde(rename = "initHash")]
    pub init_hash: Vec<u8>,

    pub hashes: Vec<ByteBuf>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct DataMap {
    pub offset: u32,
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct SubsetMap {
    pub offset: u32,
    pub length: u32,
}

/// Helper class to create BmffHash assertion. (These are auto-generated by the SDK.)
#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct BmffHash {
    exclusions: Vec<ExclusionsMap>,

    #[serde(skip_serializing_if = "Option::is_none")]
    alg: Option<String>,

    #[serde(with = "serde_bytes")]
    hash: Vec<u8>,

    #[serde(skip_serializing_if = "Option::is_none")]
    merkle: Option<Vec<MerkleMap>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<UriT>,

    #[serde(skip_deserializing, skip_serializing)]
    pub path: PathBuf,
}

impl BmffHash {
    pub fn new(name: &str, alg: &str, url: Option<UriT>) -> Self {
        BmffHash {
            exclusions: Vec::new(),
            alg: Some(alg.to_string()),
            hash: Vec::new(),
            merkle: None,
            name: Some(name.to_string()),
            url,
            path: PathBuf::new(),
        }
    }

    /// Label prefix for a BMFF hash assertion.
    ///
    /// See <https://c2pa.org/specifications/specifications/1.0/specs/C2PA_Specification.html#_bmff_based_hash>.
    pub const LABEL: &'static str = labels::BMFF_HASH;

    pub fn exclusions(&self) -> &[ExclusionsMap] {
        self.exclusions.as_ref()
    }

    pub fn exclusions_mut(&mut self) -> &mut Vec<ExclusionsMap> {
        &mut self.exclusions
    }

    pub fn alg(&self) -> Option<&String> {
        self.alg.as_ref()
    }

    pub fn hash(&self) -> &[u8] {
        self.hash.as_ref()
    }

    pub fn set_hash(&mut self, hash: Vec<u8>) {
        self.hash = hash;
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn url(&self) -> Option<&UriT> {
        self.url.as_ref()
    }

    /// Returns `true` if this is a remote hash.
    pub fn is_remote_hash(&self) -> bool {
        self.url.is_some()
    }

    pub fn set_merkle(&mut self, merkle: Vec<MerkleMap>) {
        self.merkle = Some(merkle);
    }

    /// Generate the hash value for the asset using the range from the BmffHash.
    pub fn gen_hash(&mut self, asset_path: &Path) -> Result<()> {
        self.hash = self.hash_from_asset(asset_path)?;
        self.path = PathBuf::from(asset_path);
        Ok(())
    }

    /// Generate the hash again.
    pub fn regen_hash(&mut self) -> Result<()> {
        let p = self.path.clone();
        self.hash = self.hash_from_asset(p.as_path())?;
        Ok(())
    }

    /// Generate the asset hash from a file asset using the constructed
    /// start and length values.
    fn hash_from_asset(&mut self, asset_path: &Path) -> Result<Vec<u8>> {
        if self.is_remote_hash() {
            return Err(Error::BadParam(
                "asset hash is remote, not yet supported".to_owned(),
            ));
        }

        let alg = match self.alg {
            Some(ref a) => a.clone(),
            None => "sha256".to_string(),
        };

        let bmff_exclusions = &self.exclusions;

        // convert BMFF exclusion map to flat exclusion list
        let mut data = fs::File::open(asset_path)?;
        let exclusions = bmff_to_jumbf_exclusions(&mut data, bmff_exclusions)?;

        let hash = hash_asset_by_alg(&alg, asset_path, Some(exclusions))?;

        if hash.is_empty() {
            Err(Error::BadParam("could not generate data hash".to_string()))
        } else {
            Ok(hash)
        }
    }

    pub fn verify_in_memory_hash(&self, data: &[u8], alg: Option<String>) -> Result<()> {
        let curr_alg = match &self.alg {
            Some(a) => a.clone(),
            None => match alg {
                Some(a) => a,
                None => "sha256".to_string(),
            },
        };

        let bmff_exclusions = &self.exclusions;

        let mut data_reader = Cursor::new(data);

        // convert BMFF exclusion map to flat exclusion list
        let exclusions = bmff_to_jumbf_exclusions(&mut data_reader, bmff_exclusions)?;

        if verify_by_alg(&curr_alg, &self.hash, data, Some(exclusions)) {
            Ok(())
        } else {
            Err(Error::HashMismatch("Hashes do not match".to_owned()))
        }
    }

    pub fn verify_hash(&self, asset_path: &Path, alg: Option<&str>) -> Result<()> {
        let curr_alg = alg.unwrap_or("sha256");

        let bmff_exclusions = &self.exclusions;

        // convert BMFF exclusion map to flat exclusion list
        let mut data = fs::File::open(asset_path)?;
        let exclusions = bmff_to_jumbf_exclusions(&mut data, bmff_exclusions)?;

        if verify_asset_by_alg(curr_alg, &self.hash, asset_path, Some(exclusions)) {
            Ok(())
        } else {
            Err(Error::HashMismatch("Hashes do not match".to_owned()))
        }
    }
}

impl AssertionCbor for BmffHash {}

impl AssertionBase for BmffHash {
    const LABEL: &'static str = Self::LABEL;
    const VERSION: Option<usize> = Some(ASSERTION_CREATION_VERSION);

    fn to_assertion(&self) -> Result<Assertion> {
        Self::to_cbor_assertion(self)
    }

    fn from_assertion(assertion: &Assertion) -> Result<Self> {
        Self::from_cbor_assertion(assertion)
    }
}
