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
