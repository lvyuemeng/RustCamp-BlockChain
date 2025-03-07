use std::ops::{Deref, DerefMut};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::chain::{BlockChain, blockchain_control};

use super::DbKeys;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoWConfig {
    #[serde(skip)]
    pub target_timespan: u64,
    #[serde(skip)]
    pub difficulty_adjust_interval: u64,
    #[serde(skip)]
    pub initial_difficulty: u32,
    #[serde(skip)]
    pub allow_mining_reward: bool,
    #[serde(skip)]
    pub block_reward: u64,
    // cur difficulty
    pub cur_bits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoSConfig {
    #[serde(skip)]
    pub min_stake_amount: u64,
    #[serde(skip)]
    pub stake_lock_period: u64, // pledge blocks
    #[serde(skip)]
    pub annual_interest_rate: f64,
    #[serde(skip)]
    pub validator_count: usize,
    #[serde(skip)]
    pub epoch_length: u64,
    #[serde(skip)]
    pub security_deposit: u64,
}

#[derive(Debug,Clone,Default)]
pub struct Consensus<C> {
    pub config: C,
}

impl<C> Consensus<C> {
    pub fn new(config: C) -> Self {
        Self { config }
    }
}

impl Deref for Consensus<PoWConfig> {
    type Target = PoWConfig;
    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

impl DerefMut for Consensus<PoWConfig> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.config
    }
}

impl<C: Serialize + for<'a> Deserialize<'a> + Default> Consensus<C> {
    pub fn put_state(&self, chain: &BlockChain<C>) -> Result<()> {
        chain
            .db
            .put(DbKeys::CUR_STATE, bincode::serialize(&self.config)?)
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn get_state(chain: &BlockChain<C>) -> Result<C> {
        let state = chain.db.get(DbKeys::CUR_STATE)?;
        match state {
            Some(s) => bincode::deserialize(&s).map_err(|e| anyhow::anyhow!(e)),
            None => {
                bail!("Can't found state for consensus!")
            }
        }
    }
}

impl Default for PoWConfig {
    fn default() -> Self {
        Self {
            target_timespan: blockchain_control::TARGET_TIME_SPAN,
            difficulty_adjust_interval: blockchain_control::DIFFICULTY_ADJUST_INTERVAL,
            initial_difficulty: blockchain_control::DEFAULT_DIFFICULTY,
            allow_mining_reward: true,
            block_reward: 50,
            cur_bits: blockchain_control::DEFAULT_DIFFICULTY,
        }
    }
}

impl Consensus<PoSConfig> {}
