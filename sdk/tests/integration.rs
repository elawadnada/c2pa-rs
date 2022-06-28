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

/// complete functional integration test with acquisitions and ingredients
// isolate from wasm by wrapping in module
#[cfg(feature = "file_io")]
mod integration_1 {

    use c2pa::{
        assertions::{c2pa_action, Action, Actions},
        get_signer_from_files, Ingredient, Manifest, ManifestStore, Result, Signer,
    };
    use std::path::PathBuf;
    use tempfile::tempdir;

    const GENERATOR: &str = "app";

    fn get_temp_signer() -> Box<dyn Signer> {
        // sign and embed into the target file
        let mut signcert_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        signcert_path.push("tests/fixtures/certs/ps256.pub");
        let mut pkey_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        pkey_path.push("tests/fixtures/certs/ps256.pem");
        get_signer_from_files(signcert_path, pkey_path, "ps256", None)
            .expect("get_signer_from_files")
    }

    #[test]
    #[cfg(feature = "file_io")]
    fn test_embed_manifest() -> Result<()> {
        // set up parent and destination paths
        let dir = tempdir()?;
        let output_path = dir.path().join("test_file.jpg");
        let mut parent_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        parent_path.push("tests/fixtures/earth_apollo17.jpg");
        let mut ingredient_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        ingredient_path.push("tests/fixtures/libpng-test.png");

        // create a new Manifest
        let mut manifest = Manifest::new(GENERATOR.to_owned());

        // allocate actions so we can add them
        let mut actions = Actions::new();

        // add a parent ingredient
        let parent = Ingredient::from_file(&parent_path)?;
        // add an action assertion stating that we imported this file
        actions = actions.add_action(
            Action::new(c2pa_action::EDITED)
                .set_parameter("name".to_owned(), "import")?
                .set_parameter("identifier".to_owned(), parent.instance_id().to_owned())?,
        );

        // set the parent ingredient
        manifest.set_parent(parent)?;

        // edit our image
        let mut img = image::open(&parent_path)?;
        img = img.brighten(50); // brighten the image

        actions = actions.add_action(
            Action::new("c2pa.edit").set_parameter("name".to_owned(), "brightnesscontrast")?,
        );

        // add an ingredient
        let ingredient = Ingredient::from_file(&ingredient_path)?;

        // now place an image in the image
        let img_ingredient = image::open(&ingredient_path)?;
        let img_small = img_ingredient.thumbnail(500, 500);
        image::imageops::overlay(&mut img, &img_small, 0, 0);

        // add an action assertion stating that we imported this file
        actions = actions.add_action(
            Action::new(c2pa_action::EDITED)
                .set_parameter("name".to_owned(), "import")?
                .set_parameter("identifier".to_owned(), ingredient.instance_id().to_owned())?,
            // could add other parameters for position and size here
        );

        manifest.add_ingredient(ingredient);

        manifest.add_assertion(&actions)?;

        // now place an image in the image
        let img_ingredient = image::open(&ingredient_path)?;
        let img_small = img_ingredient.thumbnail(500, 500);
        image::imageops::overlay(&mut img, &img_small, 0, 0);

        // save the edited image to our output path
        img.save(&output_path)?;

        // sign and embed into the target file
        let signer = get_temp_signer();
        manifest.embed(&output_path, &output_path, &*signer)?;

        // read our new file with embedded manifest
        let manifest_store = ManifestStore::from_file(&output_path)?;

        println!("{}", manifest_store);

        assert!(manifest_store.get_active().is_some());
        if let Some(manifest) = manifest_store.get_active() {
            assert!(manifest.title().is_some());
            assert_eq!(manifest.ingredients().len(), 2);
        } else {
            panic!("no manifest in store");
        }
        Ok(())
    }
}
