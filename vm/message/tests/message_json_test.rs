// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

#![cfg(feature = "json")]

use address::Address;
use crypto::{Signature, Signer};
use forest_message::signed_message::{
    self,
    json::{SignedMessageJson, SignedMessageJsonRef},
    SignedMessage,
};
use forest_message::unsigned_message::{
    self,
    json::{UnsignedMessageJson, UnsignedMessageJsonRef},
    UnsignedMessage,
};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::error::Error;
use vm::Serialized;

#[test]
fn unsigned_symmetric_json() {
    let message_json = r#"{"Version":9,"To":"t01234","From":"t01234","Nonce":42,"Value":"0","GasPrice":"0","GasLimit":9,"Method":1,"Params":"Ynl0ZSBhcnJheQ=="}"#;

    // Deserialize
    let UnsignedMessageJson(cid_d) = from_str(message_json).unwrap();

    // Serialize
    let ser_cid = to_string(&UnsignedMessageJsonRef(&cid_d)).unwrap();
    assert_eq!(ser_cid, message_json);
}

#[test]
fn signed_symmetric_json() {
    let message_json = r#"{"Message":{"Version":9,"To":"t01234","From":"t01234","Nonce":42,"Value":"0","GasPrice":"0","GasLimit":9,"Method":1,"Params":"Ynl0ZSBhcnJheQ=="},"Signature":{"Type":2,"Data":"Ynl0ZSBhcnJheQ=="}}"#;

    // Deserialize
    let SignedMessageJson(cid_d) = from_str(message_json).unwrap();

    // Serialize
    let ser_cid = to_string(&SignedMessageJsonRef(&cid_d)).unwrap();
    assert_eq!(ser_cid, message_json);
}

#[test]
fn message_json_annotations() {
    let unsigned = UnsignedMessage::builder()
        .to(Address::new_id(12))
        .from(Address::new_id(34))
        .sequence(5)
        .value(6u8.into())
        .method_num(7)
        .params(Serialized::default())
        .gas_limit(8)
        .gas_price(9u8.into())
        .version(10)
        .build()
        .unwrap();

    struct DummySigner;
    impl Signer for DummySigner {
        fn sign_bytes(&self, _: Vec<u8>, _: &Address) -> Result<Signature, Box<dyn Error>> {
            Ok(Signature::new_bls(vec![0u8, 1u8]))
        }
    }
    let signed = SignedMessage::new(unsigned.clone(), &DummySigner).unwrap();

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestStruct {
        #[serde(with = "unsigned_message::json")]
        unsigned: UnsignedMessage,
        #[serde(with = "signed_message::json")]
        signed: SignedMessage,
    }
    let test_json = r#"
        {
            "unsigned": {
                "Version": 10,
                "To": "t012",
                "From": "t034",
                "Nonce": 5,
                "Value": "6",
                "GasPrice": "9",
                "GasLimit": 8,
                "Method": 7,
                "Params": ""
            },
            "signed": {
                "Message": {
                    "Version": 10,
                    "To": "t012",
                    "From": "t034",
                    "Nonce": 5,
                    "Value": "6",
                    "GasPrice": "9",
                    "GasLimit": 8,
                    "Method": 7,
                    "Params": ""
                },
                "Signature": {
                    "Type": 2,
                    "Data": "AAE="
                }
            }
        }
        "#;
    let expected = TestStruct { unsigned, signed };
    assert_eq!(from_str::<TestStruct>(test_json).unwrap(), expected);
}
