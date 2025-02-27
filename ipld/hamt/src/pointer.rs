// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use super::node::Node;
use super::{Error, KeyValuePair, MAX_ARRAY_WIDTH};
use cid::Cid;
use forest_ipld::Ipld;
use serde::de::{self, DeserializeOwned};
use serde::ser;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Pointer to index values or a link to another child node.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Pointer<K> {
    Values(Vec<KeyValuePair<K>>),
    Link(Cid),
    Cache(Box<Node<K>>),
}

impl<K> Serialize for Pointer<K>
where
    K: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Pointer::Values(vals) => {
                #[derive(Serialize)]
                struct ValsSer<'a, A> {
                    #[serde(rename = "1")]
                    vals: &'a [KeyValuePair<A>],
                };
                ValsSer { vals }.serialize(serializer)
            }
            Pointer::Link(cid) => {
                #[derive(Serialize)]
                struct LinkSer<'a> {
                    #[serde(rename = "0")]
                    cid: &'a Cid,
                };
                LinkSer { cid }.serialize(serializer)
            }
            Pointer::Cache(_) => Err(ser::Error::custom("Cannot serialize cached values")),
        }
    }
}

impl<'de, K> Deserialize<'de> for Pointer<K>
where
    K: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct PointerDeser<A> {
            #[serde(rename = "1")]
            vals: Option<Vec<KeyValuePair<A>>>,

            #[serde(rename = "0")]
            cid: Option<Cid>,
        }
        let pointer_map = PointerDeser::deserialize(deserializer)?;
        match pointer_map {
            PointerDeser { vals: Some(v), .. } => Ok(Pointer::Values(v)),
            PointerDeser { cid: Some(cid), .. } => Ok(Pointer::Link(cid)),
            _ => Err(de::Error::custom("Unexpected pointer serialization")),
        }
    }
}

impl<K> Default for Pointer<K> {
    fn default() -> Self {
        Pointer::Values(Vec::new())
    }
}

impl<K> Pointer<K>
where
    K: Serialize + DeserializeOwned + Clone,
{
    pub(crate) fn from_key_value(key: K, value: Ipld) -> Self {
        Pointer::Values(vec![KeyValuePair::new(key, value)])
    }

    /// Internal method to cleanup children, to ensure consistent tree representation
    /// after deletes.
    pub(crate) fn clean(&mut self) -> Result<(), Error> {
        match self {
            Pointer::Cache(n) => match n.pointers.len() {
                0 => Err(Error::ZeroPointers),
                1 => {
                    // Node has only one pointer, swap with parent node
                    if let p @ Pointer::Values(_) = &mut n.pointers[0] {
                        // Only creating temp value to get around borrowing self mutably twice
                        let mut move_pointer = Pointer::Values(Default::default());
                        std::mem::swap(&mut move_pointer, p);
                        *self = move_pointer
                    }
                    Ok(())
                }
                2..=MAX_ARRAY_WIDTH => {
                    // Iterate over all pointers in cached node to see if it can fit all within
                    // one values node
                    let mut child_vals: Vec<KeyValuePair<K>> = Vec::with_capacity(MAX_ARRAY_WIDTH);
                    for pointer in n.pointers.iter() {
                        if let Pointer::Values(kvs) = pointer {
                            for kv in kvs.iter() {
                                if child_vals.len() == MAX_ARRAY_WIDTH {
                                    // Child values cannot be fit into parent node, keep as is
                                    return Ok(());
                                }
                                child_vals.push(kv.clone());
                            }
                        } else {
                            return Ok(());
                        }
                    }
                    // Replace link node with child values
                    *self = Pointer::Values(child_vals);
                    Ok(())
                }
                _ => Ok(()),
            },
            _ => unreachable!("clean is only called on cached pointer"),
        }
    }
}
