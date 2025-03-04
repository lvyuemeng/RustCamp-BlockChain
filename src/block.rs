use chrono::Utc;
use num_bigint::BigUint;
use sha2::{Digest, Sha256};

use crate::{hash::Hashable, transaction::Transaction};

pub struct Blocks<T: Transaction> {
    pub chain: Block<T>,
}

#[derive(Debug, Clone)]
pub struct Block<T: Transaction> {
    pub header: BlockHeader,
    txs: Vec<T>,
}

#[derive(Debug, Clone)]
pub struct BlockHeader {
    pub prev_hash: Vec<u8>,
    pub merkle_root: Vec<u8>,
    pub timestamp: i64,
    // Difficulty Goal
    pub bits: u32,
    pub nonce: u64,
}

impl Hashable for BlockHeader {
    fn hash(&self) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(self.prev_hash.clone());
        hasher.update(self.merkle_root.clone());
        hasher.update(self.timestamp.to_le_bytes());
        hasher.update(self.bits.to_le_bytes());
        hasher.update(self.nonce.to_le_bytes());
        let result = hasher.finalize();

        let mut hasher = Sha256::new();
        hasher.update(result);
        let result = hasher.finalize();
        result.to_vec()
    }
}

impl<T: Transaction> Block<T> {
    // TODO: Dynamic difficulty.
    fn validate(self, prev: &Block<T>) -> bool {
        self.header.prev_hash ==  prev.header.hash() && self.header.timestamp > prev.header.timestamp
    }

    pub fn genesis() -> Block<T> {
        Block {
            header: BlockHeader {
                prev_hash: "0".repeat(64).as_bytes().to_vec(),
                merkle_root: "0".repeat(64).as_bytes().to_vec(),
                timestamp: 1685000000,
                bits: 0x1d00ffff,
                nonce: 0,
            },
            txs: Vec::new(),
        }
    }
}

pub struct ProofWork {
    target: BigUint,
}

impl ProofWork {
    pub fn from_bits(bits: &[u8]) -> ProofWork {
        ProofWork {
            target: BigUint::from_bytes_be(bits),
        }
    }

    pub fn is_valid(&self, hash: &[u8]) -> bool {
        BigUint::from_bytes_be(hash) <= self.target
    }

    pub fn run(&self, mut bh: BlockHeader) -> BlockHeader {
        let mut nonce = 0u64;
        loop {
            bh.nonce = nonce;
            let hash = bh.hash();

            if self.is_valid(&hash) {
                return bh;
            }

            nonce += 1;
            
            if nonce % 1_000_000 == 0 {
                bh.timestamp = Utc::now().timestamp();
            }
        }
    }
}
