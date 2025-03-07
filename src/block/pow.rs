use chrono::Utc;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    block::{Block, BlockHeader, Proof, Transaction},
    chain::blockchain_control,
    hash::{Hashable, bits_to_target},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct POW {
    pub bits: u32,
    pub nonce: u64,
}

impl Hashable for POW {
    fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.bits.to_le_bytes());
        hasher.update(self.nonce.to_le_bytes());
        hasher.finalize().into()
    }
}

impl Proof for POW {
    type Config = u32;
    fn validate(&self, _prev: &Self, header_hash: &[u8]) -> bool {
        let target = bits_to_target(self.bits);
        BigUint::from_bytes_be(header_hash) <= target
    }

    fn genesis_config() -> Self {
        Self {
            bits: blockchain_control::DEFAULT_DIFFICULTY,
            nonce: 0,
        }
    }

    fn new(ctx: Self::Config) -> Self {
        Self {
            bits: ctx,
            nonce: 0,
        }
    }
}

impl POW {
    pub fn target(&self) -> BigUint {
        bits_to_target(self.bits)
    }   
    
    pub fn from_bits(bits:u32) ->  BigUint {
        bits_to_target(bits)
    }
    
    pub fn is_valid(&self, hash: &[u8]) -> bool {
        BigUint::from_bytes_be(hash) <= self.target()
    }
    
    pub fn run(&self, mut bh: BlockHeader<POW>) -> BlockHeader<POW> {
        let mut nonce = 0u64;
        loop {
            bh.proof.nonce = nonce;
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

impl<T: Transaction + Serialize> Block<T, POW> {
    pub fn mine(&mut self) {
        self.header = self.header.proof.run(self.header.clone())
    }
}
