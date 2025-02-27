// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use encoding::tuple::*;
use vm::{ExitCode, Serialized};

/// Result of a state transition from a message
#[derive(PartialEq, Clone, Serialize_tuple, Deserialize_tuple)]
pub struct MessageReceipt {
    pub exit_code: ExitCode,
    pub return_data: Serialized,
    pub gas_used: i64,
}

#[cfg(feature = "json")]
pub mod json {
    use super::*;
    use num_traits::cast::FromPrimitive;
    use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

    /// Wrapper for serializing and deserializing a SignedMessage from JSON.
    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    pub struct MessageReceiptJson(#[serde(with = "self")] pub MessageReceipt);

    /// Wrapper for serializing a SignedMessage reference to JSON.
    #[derive(Serialize)]
    #[serde(transparent)]
    pub struct MessageReceiptJsonRef<'a>(#[serde(with = "self")] pub &'a MessageReceipt);

    impl From<MessageReceiptJson> for MessageReceipt {
        fn from(wrapper: MessageReceiptJson) -> Self {
            wrapper.0
        }
    }

    pub fn serialize<S>(m: &MessageReceipt, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        #[serde(rename_all = "PascalCase")]
        struct MessageReceiptSer<'a> {
            exit_code: u64,
            #[serde(rename = "Return")]
            return_data: &'a [u8],
            gas_used: i64,
        }
        MessageReceiptSer {
            exit_code: m.exit_code as u64,
            return_data: m.return_data.bytes(),
            gas_used: m.gas_used,
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<MessageReceipt, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "PascalCase")]
        struct MessageReceiptDe {
            exit_code: u64,
            #[serde(rename = "Return")]
            return_data: Vec<u8>,
            gas_used: i64,
        }
        let MessageReceiptDe {
            exit_code,
            return_data,
            gas_used,
        } = Deserialize::deserialize(deserializer)?;
        Ok(MessageReceipt {
            exit_code: ExitCode::from_u64(exit_code).ok_or_else(|| {
                de::Error::custom("MessageReceipt deserialization: Could not turn u64 to ExitCode")
            })?,
            return_data: Serialized::new(return_data),
            gas_used,
        })
    }
}
