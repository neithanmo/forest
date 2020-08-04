// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

mod errors;
mod network;
mod payload;
mod protocol;
pub use self::errors::Error;
pub use self::network::Network;
pub use self::payload::Payload;
pub use self::protocol::Protocol;

use data_encoding::Encoding;
use data_encoding_macro::{internal_new_encoding, new_encoding};
use encoding::{blake2b_variable, de, ser, serde_bytes, Cbor};
use std::fmt;
use std::hash::Hash;
use std::str::FromStr;

/// defines the encoder for base32 encoding with the provided string with no padding
const ADDRESS_ENCODER: Encoding = new_encoding! {
    symbols: "abcdefghijklmnopqrstuvwxyz234567",
    padding: None,
};

pub const BLS_PUB_LEN: usize = 48;
pub const PAYLOAD_HASH_LEN: usize = 20;
pub const CHECKSUM_HASH_LEN: usize = 4;
const MAX_ADDRESS_LEN: usize = 84 + 2;
const MAINNET_PREFIX: &str = "f";
const TESTNET_PREFIX: &str = "t";

// TODO pull network from config (probably)
const NETWORK_DEFAULT: Network = Network::Testnet;

/// Address is the struct that defines the protocol and data payload conversion from either
/// a public key or value
#[derive(PartialEq, Eq, Clone, Debug, Hash, Copy)]
pub struct Address {
    network: Network,
    payload: Payload,
}

impl Address {
    /// Address constructor
    fn new(network: Network, protocol: Protocol, bz: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            network,
            payload: Payload::new(protocol, bz)?,
        })
    }

    /// Creates address from encoded bytes
    pub fn from_bytes(bz: &[u8]) -> Result<Self, Error> {
        if bz.len() < 2 {
            Err(Error::InvalidLength)
        } else {
            let protocol = Protocol::from_byte(bz[0]).ok_or(Error::UnknownProtocol)?;
            Self::new(NETWORK_DEFAULT, protocol, &bz[1..])
        }
    }

    /// Generates new address using ID protocol
    pub fn new_id(id: u64) -> Self {
        Self {
            network: NETWORK_DEFAULT,
            payload: Payload::ID(id),
        }
    }

    /// Generates new address using Secp256k1 pubkey
    pub fn new_secp256k1(pubkey: &[u8]) -> Self {
        Self {
            network: NETWORK_DEFAULT,
            payload: Payload::Secp256k1(address_hash(pubkey)),
        }
    }

    /// Generates new address using the Actor protocol
    pub fn new_actor(data: &[u8]) -> Self {
        Self {
            network: NETWORK_DEFAULT,
            payload: Payload::Actor(address_hash(data)),
        }
    }

    /// Generates new address using BLS pubkey
    pub fn new_bls(pubkey: &[u8]) -> Result<Self, Error> {
        if pubkey.len() != BLS_PUB_LEN {
            return Err(Error::InvalidBLSLength(pubkey.len()));
        }
        let mut key = [0u8; BLS_PUB_LEN];
        key.copy_from_slice(pubkey);
        Ok(Self {
            network: NETWORK_DEFAULT,
            payload: Payload::BLS(key.into()),
        })
    }

    /// Returns protocol for Address
    pub fn protocol(&self) -> Protocol {
        Protocol::from(self.payload)
    }

    /// Returns the `Payload` object from the address, where the respective protocol data is kept
    /// in an enum separated by protocol
    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Returns the raw bytes data payload of the Address
    pub fn payload_bytes(&self) -> Vec<u8> {
        self.payload.to_raw_bytes()
    }

    /// Returns network configuration of Address
    pub fn network(&self) -> Network {
        self.network
    }

    /// Sets the network for the address and returns a mutable reference to it
    pub fn set_network(&mut self, network: Network) -> &mut Self {
        self.network = network;
        self
    }

    /// Returns encoded bytes of Address
    pub fn to_bytes(&self) -> Vec<u8> {
        self.payload.to_bytes()
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", encode(self))
    }
}

impl FromStr for Address {
    type Err = Error;
    fn from_str(addr: &str) -> Result<Self, Error> {
        if addr.len() > MAX_ADDRESS_LEN || addr.len() < 3 {
            return Err(Error::InvalidLength);
        }
        // ensure the network character is valid before converting
        let network: Network = match addr.get(0..1).ok_or(Error::UnknownNetwork)? {
            TESTNET_PREFIX => Network::Testnet,
            MAINNET_PREFIX => Network::Mainnet,
            _ => {
                return Err(Error::UnknownNetwork);
            }
        };

        // get protocol from second character
        let protocol: Protocol = match addr.get(1..2).ok_or(Error::UnknownProtocol)? {
            "0" => Protocol::ID,
            "1" => Protocol::Secp256k1,
            "2" => Protocol::Actor,
            "3" => Protocol::BLS,
            _ => {
                return Err(Error::UnknownProtocol);
            }
        };

        // bytes after the protocol character is the data payload of the address
        let raw = addr.get(2..).ok_or(Error::InvalidPayload)?;
        if protocol == Protocol::ID {
            if raw.len() > 20 {
                // 20 is max u64 as string
                return Err(Error::InvalidLength);
            }
            let id = raw.parse::<u64>()?;
            return Ok(Address::new_id(id));
        }

        // decode using byte32 encoding
        let mut payload = ADDRESS_ENCODER.decode(raw.as_bytes())?;
        // payload includes checksum at end, so split after decoding
        let cksm = payload.split_off(payload.len() - CHECKSUM_HASH_LEN);

        // sanity check to make sure address hash values are correct length
        if (protocol == Protocol::Secp256k1 || protocol == Protocol::Actor)
            && payload.len() != PAYLOAD_HASH_LEN
        {
            return Err(Error::InvalidPayload);
        }

        // sanity check to make sure bls pub key is correct length
        if protocol == Protocol::BLS && payload.len() != BLS_PUB_LEN {
            return Err(Error::InvalidPayload);
        }

        // validate checksum
        let mut ingest = payload.clone();
        ingest.insert(0, protocol as u8);
        if !validate_checksum(&ingest, cksm) {
            return Err(Error::InvalidChecksum);
        }

        Address::new(network, protocol, &payload)
    }
}

impl ser::Serialize for Address {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let address_bytes = self.to_bytes();
        serde_bytes::Serialize::serialize(&address_bytes, s)
    }
}

impl<'de> de::Deserialize<'de> for Address {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let bz: Vec<u8> = serde_bytes::Deserialize::deserialize(deserializer)?;

        // Create and return created address of unmarshalled bytes
        Address::from_bytes(&bz).map_err(de::Error::custom)
    }
}

impl Cbor for Address {}

/// encode converts the address into a string
fn encode(addr: &Address) -> String {
    match addr.protocol() {
        Protocol::Secp256k1 | Protocol::Actor | Protocol::BLS => {
            let ingest = addr.to_bytes();
            let mut bz = addr.payload_bytes();

            // payload bytes followed by calculated checksum
            bz.extend(checksum(&ingest));
            format!(
                "{}{}{}",
                addr.network.to_prefix(),
                addr.protocol().to_string(),
                ADDRESS_ENCODER.encode(bz.as_mut()),
            )
        }
        Protocol::ID => format!(
            "{}{}{}",
            addr.network.to_prefix(),
            addr.protocol().to_string(),
            from_leb_bytes(&addr.payload_bytes()).expect("should read encoded bytes"),
        ),
    }
}

pub(crate) fn to_leb_bytes(id: u64) -> Result<Vec<u8>, Error> {
    let mut buf = Vec::new();

    // write id to buffer in leb128 format
    leb128::write::unsigned(&mut buf, id)?;

    // Create byte vector from buffer
    Ok(buf)
}

pub(crate) fn from_leb_bytes(bz: &[u8]) -> Result<u64, Error> {
    let mut readable = &bz[..];

    // write id to buffer in leb128 format
    Ok(leb128::read::unsigned(&mut readable)?)
}

/// Checksum calculates the 4 byte checksum hash
pub fn checksum(ingest: &[u8]) -> Vec<u8> {
    blake2b_variable(ingest, CHECKSUM_HASH_LEN)
}

/// Validates the checksum against the ingest data
pub fn validate_checksum(ingest: &[u8], expect: Vec<u8>) -> bool {
    let digest = checksum(ingest);
    digest == expect
}

/// Returns an address hash for given data
fn address_hash(ingest: &[u8]) -> [u8; 20] {
    let digest = blake2b_variable(ingest, PAYLOAD_HASH_LEN);
    let mut hash = [0u8; 20];
    hash.clone_from_slice(&digest);
    hash
}
