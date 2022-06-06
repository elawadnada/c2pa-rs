use crate::{
    assertion::{AssertionBase, AssertionDecodeError},
    error::{Error, Result},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize}; //,  Deserializer, Serializer};
use serde_json::Value;

/// Assertions in C2PA can be stored in several formats
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum ManifestAssertionKind {
    Cbor,
    Json,
    Binary,
    Uri,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(untagged)]
enum ManifestData {
    Json(Value),     // { label: String, instance: usize, data: Value },
    Binary(Vec<u8>), // ) { label: String, instance: usize, data: Value },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
/// A labeled container for an Assertion value in a Manifest
pub struct ManifestAssertion {
    /// An assertion label in reverse domain format
    label: String,
    /// The data of the assertion as Value
    //#[serde(deserialize_with = "manifest_data_deserialize", serialize_with = "manifest_data_serialize")]
    data: ManifestData,
    /// There can be more than one assertion for any label
    #[serde(skip_serializing_if = "Option::is_none")]
    instance: Option<usize>,
    /// The [ManifestAssertionKind] for this assertion (as stored in c2pa content)
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<ManifestAssertionKind>,
}

// fn manifest_data_serialize<S>(x: &ManifestData, s: S) -> std::result::Result<S::Ok, S::Error>
// where
//     S: Serializer,
// {
//     s.serialize_str("<omitted>")
// }

// fn manifest_data_deserialize<'de, D>(deserializer: D) -> std::result::Result<ManifestData, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let s: &str = Deserialize::deserialize(deserializer)?;
//     serde_json::from_str(s).map_err(D::Erro)
// }

impl ManifestAssertion {
    /// Create with label and value
    pub fn new(label: String, data: Value) -> Self {
        Self {
            label,
            data: ManifestData::Json(data),
            instance: None,
            kind: None,
        }
    }

    /// Returns the la
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn label_with_instance(&self) -> String {
        match self.instance {
            Some(i) if i > 1 => format!("{}__{}", self.label, i),
            _ => self.label.to_owned(),
        }
    }

    pub fn value(&self) -> Result<&Value> {
        match &self.data {
            ManifestData::Json(d) => Ok(d),
            ManifestData::Binary(_) => Err(Error::UnsupportedType),
        }
    }

    pub fn binary(&self) -> Result<&[u8]> {
        match &self.data {
            ManifestData::Json(_) => Err(Error::UnsupportedType),
            ManifestData::Binary(b) => Ok(b),
        }
    }

    pub fn instance(&self) -> usize {
        self.instance.unwrap_or(1)
    }

    pub fn kind(&self) -> &ManifestAssertionKind {
        match self.kind.as_ref() {
            Some(kind) => kind,
            None => &ManifestAssertionKind::Cbor,
        }
    }

    pub(crate) fn set_instance(mut self, instance: usize) -> Self {
        self.instance = if instance > 1 { Some(instance) } else { None };
        self
    }

    pub fn set_kind(mut self, kind: ManifestAssertionKind) -> Self {
        self.kind = Some(kind);
        self
    }

    pub fn from_labeled_assertion<S: Into<String>, T: Serialize>(
        label: S,
        data: &T,
    ) -> Result<Self> {
        Ok(Self::new(
            label.into(),
            serde_json::to_value(data).map_err(|_err| Error::AssertionEncoding)?,
        ))
    }

    pub fn from_helper<T: Serialize + AssertionBase>(data: &T) -> Result<Self> {
        Ok(Self::new(
            data.label().to_owned(),
            serde_json::to_value(data).map_err(|_err| Error::AssertionEncoding)?,
        ))
    }

    pub fn to_helper<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_value(self.value()?.to_owned()).map_err(|e| {
            Error::AssertionDecoding(AssertionDecodeError::from_json_err(
                self.label.to_owned(),
                None,
                "application/json".to_owned(),
                e,
            ))
        })
    }

    // pub fn to_assertion(&self) -> Result<Assertion> {
    //     match self.kind() {
    //         ManifestAssertionKind::Cbor =>
    //             Ok(UserCbor::new(self.label(), serde_cbor::to_vec(&self.value()?)?).to_assertion()?),
    //         ManifestAssertionKind::Json =>
    //             Ok(User::new(self.label(), &serde_json::to_string(&self.value()?)?).to_assertion()?),
    //         _ => Err(Error::AssertionEncoding)
    //     }
    // }
}

#[cfg(test)]
pub(crate) mod tests {
    #![allow(clippy::expect_used)]
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::assertions::{c2pa_action, Action, Actions};

    #[test]
    fn test_manifest_assertion() {
        let actions = Actions::new().add_action(Action::new(c2pa_action::EDITED));
        let value = serde_json::to_value(actions).unwrap();
        let mut ma = ManifestAssertion::new(Actions::LABEL.to_owned(), value);
        assert_eq!(ma.label(), Actions::LABEL);

        ma = ma.set_instance(1);
        assert_eq!(ma.instance, None);
        ma = ma.set_instance(2);
        assert_eq!(ma.instance(), 2);
        assert_eq!(ma.kind(), &ManifestAssertionKind::Cbor);
        ma = ma.set_kind(ManifestAssertionKind::Json);
        assert_eq!(ma.kind(), &ManifestAssertionKind::Json);

        let actions = Actions::new().add_action(Action::new(c2pa_action::EDITED));
        let ma2 = ManifestAssertion::from_helper(&actions).expect("from_assertion");
        let actions2: Actions = ma2.to_helper().expect("to_assertion");
        let actions3 = ManifestAssertion::from_labeled_assertion("foo".to_owned(), &actions2)
            .expect("from_labeled_assertion");
        assert_eq!(actions3.label(), "foo");
    }
}
