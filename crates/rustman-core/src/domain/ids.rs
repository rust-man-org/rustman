//! Typed ids, so a collection id can't be passed where a request id belongs.
//! Each is a UUID string on disk.

use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// A new random id.
            pub fn fresh() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            /// The id as stored.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(id: String) -> Self {
                Self(id)
            }
        }

        impl From<&str> for $name {
            fn from(id: &str) -> Self {
                Self(id.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl PartialEq<str> for $name {
            fn eq(&self, other: &str) -> bool {
                self.0 == other
            }
        }

        impl PartialEq<&str> for $name {
            fn eq(&self, other: &&str) -> bool {
                self.0 == *other
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id!(
    /// Identifies a saved request.
    RequestId
);
id!(
    /// Identifies a collection.
    CollectionId
);
id!(
    /// Identifies a group (folder) inside a collection.
    GroupId
);
id!(
    /// Identifies an environment.
    EnvId
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_serialize_as_plain_strings() {
        let id = RequestId::from("req-1");
        assert_eq!(serde_json::to_string(&id).unwrap(), "\"req-1\"");
        let back: RequestId = serde_json::from_str("\"req-1\"").unwrap();
        assert_eq!(back, id);
        assert_eq!(back, "req-1");
    }
}
