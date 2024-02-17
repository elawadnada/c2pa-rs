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

//! Example App showing how to use the new v2 API
use std::io::{Cursor, Seek};

use anyhow::Result;
use c2pa::{create_callback_signer, Builder, C2pa, SignerCallback, SigningAlg};
use serde_json::json;

const TEST_IMAGE: &[u8] = include_bytes!("../tests/fixtures/CA.jpg");
const CERTS: &[u8] = include_bytes!("../tests/fixtures/certs/ed25519.pub");
const PRIVATE_KEY: &[u8] = include_bytes!("../tests/fixtures/certs/ed25519.pem");

fn manifest_def(title: &str, format: &str) -> String {
    json!({
        "title": title,
        "format": format,
        "claim_generator_info": [
            {
                "name": "c2pa test",
                "version": env!("CARGO_PKG_VERSION")
            }
        ],
        "thumbnail": {
            "format": format,
            "identifier": "manifest_thumbnail.jpg"
        },
        "ingredients": [
            {
                "title": "Test",
                "format": "image/jpeg",
                "instance_id": "12345",
                "relationship": "inputTo"
            }
        ],
        "assertions": [
            {
                "label": "c2pa.actions",
                "data": {
                    "actions": [
                        {
                            "action": "c2pa.edited",
                            "digitalSourceType": "http://cv.iptc.org/newscodes/digitalsourcetype/trainedAlgorithmicMedia",
                            "softwareAgent": {
                                "name": "My AI Tool",
                                "version": "0.1.0"
                            }
                        }
                    ]
                }
            }
        ]
    }).to_string()
}

/// This example demonstrates how to use the new v2 API to create a manifest store
/// It uses only streaming apis, showing how to avoid file i/o
/// This example uses the `ed25519` signing algorithm
fn main() -> Result<()> {
    let c2pa = C2pa::new();

    let title = "v2_edited.jpg";
    let format = "image/jpeg";
    let parent_name = "CA.jpg";
    let mut source = Cursor::new(TEST_IMAGE);

    let json = manifest_def(title, format);

    let mut builder = c2pa.builder();
    builder.with_json(&json)?.add_ingredient(
        json!({
            "title": parent_name,
            "relationship": "parentOf"
        })
        .to_string(),
        format,
        &mut source,
    )?;

    let thumb_uri = builder.thumbnail.as_ref().map(|t| t.identifier.clone());

    // add a manifest thumbnail ( just reuse the image for now )
    if let Some(uri) = thumb_uri {
        if !uri.starts_with("self#jumbf") {
            source.rewind()?;
            builder.add_resource(&uri, &mut source)?;
        }
    }

    // write the manifest builder to a zipped stream
    let mut zipped = Cursor::new(Vec::new());
    builder.zip(&mut zipped)?;

    // write the zipped stream to a file for debugging
    //let debug_path = format!("{}/../target/test.zip", env!("CARGO_MANIFEST_DIR"));
    // std::fs::write(debug_path, zipped.get_ref())?;

    // unzip the manifest builder from the zipped stream
    zipped.rewind()?;

    //let signer = create_signer::from_keys(CERTS, PRIVATE_KEY, SigningAlg::Es256, None)?;
    let ed_signer = Box::new(EdCallbackSigner {});
    let signer = create_callback_signer(SigningAlg::Ed25519, CERTS, ed_signer, None)?;

    let mut builder = Builder::unzip(&mut zipped)?;
    // sign the ManifestStoreBuilder and write it to the output stream
    let mut dest = Cursor::new(Vec::new());
    builder.sign(format, &mut source, &mut dest, signer.as_ref())?;

    // read and validate the signed manifest store
    dest.rewind()?;

    let reader = c2pa.read(format, &mut dest)?;

    // extract a thumbnail image from the ManifestStore
    let mut thumbnail = Cursor::new(Vec::new());
    if let Some(manifest) = reader.active_manifest() {
        if let Some(thumbnail_ref) = manifest.thumbnail_ref() {
            reader.resource(&thumbnail_ref.identifier, &mut thumbnail)?;
            println!(
                "wrote thumbnail {} of size {}",
                thumbnail_ref.format,
                thumbnail.get_ref().len()
            );
        }
    }

    println!("{}", reader.json());
    assert!(reader.status().is_none());
    assert_eq!(reader.active_manifest().unwrap().title().unwrap(), title);

    Ok(())
}

struct EdCallbackSigner {}

impl SignerCallback for EdCallbackSigner {
    fn sign(&self, data: &[u8]) -> c2pa::Result<Vec<u8>> {
        ed_sign(data, PRIVATE_KEY)
    }
}

fn ed_sign(data: &[u8], private_key: &[u8]) -> c2pa::Result<Vec<u8>> {
    use ed25519_dalek::{Keypair, PublicKey, SecretKey, Signature, Signer};
    use pem::parse;

    // Parse the PEM data to get the private key
    let pem = parse(private_key).map_err(|e| c2pa::Error::OtherError(Box::new(e)))?;
    // For Ed25519, the key is 32 bytes long, so we skip the first 16 bytes of the PEM data
    let key_bytes = &pem.contents()[16..];
    let secret =
        SecretKey::from_bytes(key_bytes).map_err(|e| c2pa::Error::OtherError(Box::new(e)))?;
    let public = PublicKey::from(&secret);
    // Create a keypair from the secret and public keys
    let keypair = Keypair { secret, public };
    // Sign the data
    let signature: Signature = keypair.sign(data);

    Ok(signature.to_bytes().to_vec())
}

// #[cfg(feature = "openssl")]
// use openssl::{error::ErrorStack, pkey::PKey};
// #[cfg(feature = "openssl")]
// fn ed_sign(data: &[u8], pkey: &[u8]) -> std::result::Result<Vec<u8>, ErrorStack> {
//     let pkey = PKey::private_key_from_pem(pkey)?;
//     let mut signer = openssl::sign::Signer::new_without_digest(&pkey)?;
//     signer.sign_oneshot_to_vec(data)
// }

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    use super::*;

    #[cfg_attr(not(target_arch = "wasm32"), actix::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_v2_api() -> Result<()> {
        main()
    }
}
