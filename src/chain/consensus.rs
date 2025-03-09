use std::ops::{Deref, DerefMut};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    block::Consensus,
    chain::{BlockChain, DbKeys},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ConsensusData<P: Consensus> {
    pub config: P,
}

impl<P: Consensus> ConsensusData<P> {
    pub fn new(config: P) -> Self {
        Self { config }
    }
}

impl<P: Consensus> Deref for ConsensusData<P> {
    type Target = P;
    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl<P: Consensus> DerefMut for ConsensusData<P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

impl<P: Consensus> ConsensusData<P> {
    pub fn put_state(&self, chain: &BlockChain<P>) -> Result<()> {
        chain
            .db
            .put(
                DbKeys::CUR_STATE,
                bincode::serde::encode_to_vec(self, bincode::config::standard())?,
            )
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn get_state(chain: &BlockChain<P>) -> Result<P::Data> {
        let state = chain.db.get(DbKeys::CUR_STATE)?;
        match state {
            Some(s) => {
                let s = bincode::serde::decode_from_slice(&s, bincode::config::standard())
                    .map_err(|e| anyhow::anyhow!(e))?;
                Ok(s.0)
            }
            None => {
                bail!("Can't found state for consensus!")
            }
        }
    }
}
