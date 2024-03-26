// at your option.

// Unless required by applicable law or agreed to in writing,
// this software is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR REPRESENTATIONS OF ANY KIND, either express or
// implied. See the LICENSE-MIT and LICENSE-APACHE files for the
// specific language governing permissions and limitations under
// each license.

/// complete functional integration test with acquisitions and ingredients
// isolate from wasm by wrapping in module
mod integration_v2 {

    use std::io::{Cursor, Seek};

    use anyhow::Result;
    use c2pa::{create_callback_signer, Builder, Reader, SigningAlg};
    use serde_json::json;

    const PARENT_JSON: &str = r#"
    {
        "title": "Parent Test",
        "format": "image/jpeg",
        "relationship": "parentOf"
    }
    "#;

    const TEST_IMAGE: &[u8] = include_bytes!("../tests/fixtures/CA.jpg");
    const CERTS: &[u8] = include_bytes!("../tests/fixtures/certs/ed25519.pub");
    const PRIVATE_KEY: &[u8] = include_bytes!("../tests/fixtures/certs/ed25519.pem");

    fn get_manifest_def(title: &str, format: &str) -> String {
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
            "format": "image/jpeg",
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
                            "softwareAgent": "Adobe Firefly 0.1.0"
                        }
                    ]
                }
            }
        ]
    }).to_string()
    }

    //#[cfg(not(target_arch = "wasm32"))]
    fn main() -> Result<()> {
        let title = "CA.jpg";
        let format = "image/jpeg";
        let mut source = Cursor::new(TEST_IMAGE);

        let json = get_manifest_def(title, format);

        // don't try to verify on wasm since it doesn't support ed25519 yet

        let mut builder = Builder::from_json(&json)?;
        builder.add_ingredient(PARENT_JSON, format, &mut source)?;

        // add a manifest thumbnail ( just reuse the image for now )
        source.rewind()?;
        builder.add_resource("manifest_thumbnail.jpg", &mut source)?;

        // write the manifest builder to a zipped stream
        let mut zipped = Cursor::new(Vec::new());
        builder.zip(&mut zipped)?;

        // write the zipped stream to a file for debugging
        //let debug_path = format!("{}/../target/test.zip", env!("CARGO_MANIFEST_DIR"));
        // std::fs::write(debug_path, zipped.get_ref())?;

        // unzip the manifest builder from the zipped stream
        zipped.rewind()?;

        let mut dest = {
            let ed_signer = |data: &[u8]| ed_sign(data, PRIVATE_KEY);
            let signer = create_callback_signer(SigningAlg::Ed25519, CERTS, ed_signer, None)?;

            let mut builder = Builder::unzip(&mut zipped)?;
            // sign the ManifestStoreBuilder and write it to the output stream
            let mut dest = Cursor::new(Vec::new());
            builder.sign(format, &mut source, &mut dest, signer.as_ref())?;

            // read and validate the signed manifest store
            dest.rewind()?;
            dest
        };

        let reader = Reader::from_stream(format, &mut dest)?;

        // extract a thumbnail image from the ManifestStore
        let mut thumbnail = Cursor::new(Vec::new());
        if let Some(manifest) = reader.active() {
            if let Some(thumbnail_ref) = manifest.thumbnail_ref() {
                reader.resource_to_stream(&thumbnail_ref.identifier, &mut thumbnail)?;
                println!(
                    "wrote thumbnail {} of size {}",
                    thumbnail_ref.format,
                    thumbnail.get_ref().len()
                );
            }
        }

        println!("{}", reader.json());
        #[cfg(not(target_arch = "wasm32"))] // todo: remove this check when wasm supports ed25519
        assert!(reader.validation_status().is_none());
        assert_eq!(reader.active().unwrap().title().unwrap(), title);

        Ok(())
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

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(not(target_arch = "wasm32"), actix::test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    async fn test_v2_api() -> Result<()> {
        main()
    }
}
