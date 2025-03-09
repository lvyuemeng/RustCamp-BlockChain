use std::fmt::Display;

use chrono::Utc;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use anyhow::{Result,bail};

use crate::{
    block::{Block, BlockHeader, Consensus, Transaction},
    chain::blockchain_control,
    hash::{Hashable, bits_to_target},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoW {
    pub target_timespan: u64,
    pub difficulty_adjust_interval: u64,
    pub initial_difficulty: u32,
    pub allow_mining_reward: bool,
    pub block_reward: u64,
    // cur difficulty
    pub cur_bits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoWData {
    pub bits: u32,
    pub nonce: u64,
}

impl Display for PoWData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PoW\n bits: {}\n nonce: {}", self.bits, self.nonce)
    }
}


impl Default for PoW {
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

impl Consensus for PoW {
    type Data = PoWData;
    fn validate<T:Transaction>(&self, block:&Block<T,Self>) -> bool {
        let target = bits_to_target(self.cur_bits);
        BigUint::from_bytes_be(&block.header.hash()) <= target
    }
    
    fn generate_block<T:Transaction>(&mut self,prev:&Block<T,Self>,txs:super::Transactions<T>) -> Result<Block<T,Self>> {
        let merkle_root = match txs.merkle_root() {
            Some(root) => root,
            None => bail!("No merkle root found!"),
        };

        let data = PoWData {
            bits: self.cur_bits,
            nonce: 0,
        };
        let block = Block {
            header: BlockHeader {
                prev_hash: prev.header.hash().to_vec(),
                merkle_root,
                timestamp: Utc::now().timestamp(),
                data,
            },
            txs,
        };
        Ok(block)
    }
    
    fn genesis_data() -> Self::Data {
        PoWData {
            bits: blockchain_control::DEFAULT_DIFFICULTY,
            nonce: 0,
        }
    }
    
}

impl PoWData {
    pub fn target(&self) -> BigUint {
        bits_to_target(self.bits)
    }

    pub fn is_valid(&self, hash: &[u8]) -> bool {
        BigUint::from_bytes_be(hash) <= self.target()
    }

    pub fn run(&self, mut bh: BlockHeader<PoWData>) -> BlockHeader<PoWData> {
        let mut nonce = 0u64;
        loop {
            bh.data.nonce = nonce;
            let hash = bh.hash();

            if self.is_valid(&hash) {
                log::debug!("Found block with nonce {}", nonce);
                return bh;
            }

            nonce = nonce.wrapping_add(1);

            if nonce % 1_000_000 == 0 {
                bh.timestamp = Utc::now().timestamp();
                log::debug!("Retrying with timestamp {}", bh.timestamp);
            }
        }
    }
}

impl<T: Transaction + Serialize> Block<T, PoW> {
    pub fn mine(&mut self) {
        self.header = self.header.data.run(self.header.clone())
    }
}
