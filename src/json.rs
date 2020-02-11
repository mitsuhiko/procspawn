use serde::de::{self, Deserialize, DeserializeOwned, Deserializer};
use serde::ser::{self, Serialize, Serializer};

/// Utility wrapper to force values through JSON serialization.
///
/// By default `procspawn` will use [`bincode`](https://github.com/servo/bincode) to serialize
/// data across process boundaries.  This has some limitations which can cause serialization
/// or deserialization to fail for some types.
///
/// Since JSON is generally better supported in the serde ecosystem this lets you work
/// around some known bugs.
///
/// * serde flatten not being supported: [bincode#245](https://github.com/servo/bincode/issues/245)
/// * vectors with unknown length not supported: [bincode#167](https://github.com/servo/bincode/issues/167)
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Json<T>(pub T);

impl<T: Serialize> Serialize for Json<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let json = serde_json::to_string(&self.0).map_err(|e| ser::Error::custom(e.to_string()))?;
        serializer.serialize_str(&json)
    }
}

impl<'de, T: DeserializeOwned> Deserialize<'de> for Json<T> {
    fn deserialize<D>(deserializer: D) -> Result<Json<T>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json =
            String::deserialize(deserializer).map_err(|e| de::Error::custom(e.to_string()))?;
        Ok(Json(
            serde_json::from_str(&json).map_err(|e| de::Error::custom(e.to_string()))?,
        ))
    }
}
