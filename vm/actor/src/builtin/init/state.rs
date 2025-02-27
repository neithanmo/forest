// Copyright 2020 ChainSafe Systems
// SPDX-License-Identifier: Apache-2.0, MIT

use crate::{make_map_with_root, FIRST_NON_SINGLETON_ADDR};
use address::{Address, Protocol};
use cid::Cid;
use encoding::tuple::*;
use encoding::Cbor;
use ipld_blockstore::BlockStore;
use ipld_hamt::Error as HamtError;
use vm::ActorID;

/// State is reponsible for creating
#[derive(Serialize_tuple, Deserialize_tuple)]
pub struct State {
    pub address_map: Cid,
    pub next_id: ActorID,
    pub network_name: String,
}

impl State {
    pub fn new(address_map: Cid, network_name: String) -> Self {
        Self {
            address_map,
            next_id: FIRST_NON_SINGLETON_ADDR,
            network_name,
        }
    }

    /// Allocates a new ID address and stores a mapping of the argument address to it.
    /// Returns the newly-allocated address.
    pub fn map_address_to_new_id<BS: BlockStore>(
        &mut self,
        store: &BS,
        addr: &Address,
    ) -> Result<Address, HamtError> {
        let id = self.next_id;
        self.next_id += 1;

        let mut map = make_map_with_root(&self.address_map, store)?;
        map.set(addr.to_bytes().into(), id)?;
        self.address_map = map.flush()?;

        Ok(Address::new_id(id))
    }

    /// ResolveAddress resolves an address to an ID-address, if possible.
    /// If the provided address is an ID address, it is returned as-is.
    /// This means that ID-addresses (which should only appear as values, not keys)
    /// and singleton actor addresses pass through unchanged.
    ///
    /// Post-condition: all addresses succesfully returned by this method satisfy
    /// `addr.protocol() == Protocol::ID`.
    pub fn resolve_address<BS: BlockStore>(
        &self,
        store: &BS,
        addr: &Address,
    ) -> Result<Option<Address>, String> {
        if addr.protocol() == Protocol::ID {
            return Ok(Some(*addr));
        }

        let map = make_map_with_root(&self.address_map, store)?;

        Ok(map
            .get::<_, ActorID>(&addr.to_bytes())?
            .map(Address::new_id))
    }
}

impl Cbor for State {}
