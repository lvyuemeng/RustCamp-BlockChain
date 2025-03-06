use std::ops::Deref;

use chrono::Utc;
use num_bigint::BigUint;
use rs_merkle::{MerkleTree, algorithms::Sha256 as MerkleSha256};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    chain::blockchain_control,
    hash::{Hashable, bits_to_target},
    transaction::Transaction,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block<T: Transaction + Serialize> {
    pub header: BlockHeader,
    txs: Vec<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader {
    pub prev_hash: Vec<u8>,
    pub merkle_root: Vec<u8>,
    pub timestamp: i64,
    // Difficulty Goal
    pub bits: u32,
    pub nonce: u64,
}

impl Hashable for BlockHeader {
    fn hash(&self) -> [u8; 32] {
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
        result.into()
    }
}

impl<T: Transaction + Serialize> Block<T> {
    pub fn validate(&self, prev: &Block<T>) -> bool {
        let prev_valid = self.header.prev_hash == prev.header.hash();
        let time_valid = self.header.timestamp > prev.header.timestamp;
        let merkle_valid = {
            let Some(calc) = self.merkle_root() else {
                return false;
            };
            calc == self.header.merkle_root
        };

        let pow_valid = {
            let pow = ProofWork::from_bits(self.header.bits);
            pow.is_valid(&self.header.hash())
        };

        prev_valid && time_valid && merkle_valid && pow_valid
    }

    pub fn genesis() -> Block<T> {
        Block {
            header: BlockHeader {
                prev_hash: "0".repeat(64).as_bytes().to_vec(),
                merkle_root: "0".repeat(64).as_bytes().to_vec(),
                timestamp: 1685000000,
                bits: blockchain_control::DEFAULT_DIFFICULTY,
                nonce: 0,
            },
            txs: Vec::new(),
        }
    }

    pub fn mine(&mut self) {
        let pow = ProofWork::from_bits(self.header.bits);
        self.header = pow.run(self.header.clone())
    }

    fn merkle_root(&self) -> Option<Vec<u8>> {
        let leaves: Vec<[u8; 32]> = self.txs.iter().map(|tx| tx.hash()).collect();
        let mt: MerkleTree<MerkleSha256> = MerkleTree::from_leaves(&leaves);
        let root = mt.root();
        root.map(|x| x.to_vec())
    }
}

pub struct ProofWork {
    target: BigUint,
    pub test: bool,
}

impl ProofWork {
    pub fn target(&self) -> BigUint {
        self.target.clone()
    }

    pub fn test_bits(bits: u32) -> Self {
        let mut pow = ProofWork::from_bits(bits);
        pow.test = true;
        pow
    }

    pub fn from_bits(bits: u32) -> Self {
        let target = bits_to_target(bits);

        ProofWork {
            target,
            test: false,
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
                if self.test {
                    log::debug!("Found block with nonce {}", nonce);
                }
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

impl Deref for ProofWork {
    type Target = BigUint;
    fn deref(&self) -> &Self::Target {
        &self.target
    }
}
