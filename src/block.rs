use std::{fmt::{self, Display, Formatter}, ops::Deref};

use anyhow::{bail,Result};
use chrono::Utc;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    chain::blockchain_control,
    hash::{Hashable, bits_to_target},
    transaction::{Transaction, Transactions},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block<T: Transaction + Serialize> {
    pub header: BlockHeader,
    pub txs: Transactions<T>,
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
    pub fn new(prev: &Block<T>, txs: Transactions<T>, bits: u32) -> Result<Block<T>> {
        let merkle_root = match txs.merkle_root() {
            Some(root) => root,
            None => bail!("No merkle root found!"),
        };
        let block = Block {
            header: BlockHeader {
                prev_hash: prev.header.hash().to_vec(),
                merkle_root,
                timestamp: Utc::now().timestamp(),
                bits,
                nonce: 0,
            },
            txs,
        };
        Ok(block)
    }

    pub fn validate(&self, prev: &Block<T>) -> bool {
        let prev_valid = self.header.prev_hash == prev.header.hash();
        eprintln!("prev_valid: {}", prev_valid);
        let time_valid = self.header.timestamp > prev.header.timestamp;
        eprintln!("time_valid: {}", time_valid);
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
        eprintln!("pow_valid: {}", pow_valid);

        prev_valid && time_valid && merkle_valid && pow_valid
    }

    pub fn genesis<H: Transaction + Default + Serialize>() -> Block<H> {
        Block {
            header: BlockHeader {
                prev_hash: "0".repeat(64).as_bytes().to_vec(),
                merkle_root: "0".repeat(64).as_bytes().to_vec(),
                timestamp: 1685000000,
                bits: blockchain_control::DEFAULT_DIFFICULTY,
                nonce: 0,
            },
            txs: Transactions::<H>::test_new(),
        }
    }

    pub fn mine(&mut self) {
        let pow = ProofWork::from_bits(self.header.bits);
        self.header = pow.run(self.header.clone())
    }

    fn merkle_root(&self) -> Option<Vec<u8>> {
        self.txs.merkle_root()
    }
}

 impl Display for BlockHeader {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let s = format!("
            block:\n\
            \ttimestamp: {}\n\
            \tbits: {}\n\
            \tnonce: {}\n\
        ",self.timestamp,self.bits,self.nonce);
        write!(f,"{}",s)
    }
}

impl<T:Transaction+Serialize>  Display for Block<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.header)
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
