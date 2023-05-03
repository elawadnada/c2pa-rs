use anyhow::{Context, Result};

use c2pa::ManifestStore;
use schemars::gen::SchemaSettings;

fn main() -> Result<()> {
    println!("Exporting JSON schema");
    let settings = SchemaSettings::draft07();
    let gen = settings.into_generator();
    let schema = gen.into_root_schema_for::<ManifestStore>();
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
    Ok(())
}
