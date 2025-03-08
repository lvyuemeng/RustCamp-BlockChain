pub mod pos;
pub mod pow;
use std::fmt::{self, Display, Formatter};

use anyhow::{Result, bail};
use chrono::Utc;
use log::debug;
use rs_merkle::{MerkleTree, algorithms::Sha256 as MerkleSha256};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::hash::Hashable;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block<T: Transaction, H: Proof> {
    pub header: BlockHeader<H>,
    pub txs: Transactions<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockHeader<H: Proof> {
    pub prev_hash: Vec<u8>,
    pub merkle_root: Vec<u8>,
    pub timestamp: i64,
    pub proof: H,
}

pub trait Proof: Hashable + Serialize + Clone {
    type Config: Clone;
    fn validate(&self, prev: &Self, header_hash: &[u8]) -> bool;
    fn genesis_config() -> Self;
    fn new(ctx: Self::Config) -> Self;
}

pub trait Transaction: Hashable + Serialize {
    fn verify(&self) -> bool {
        false
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct DummyTransaction;

impl Hashable for DummyTransaction {
    fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"Dummy");
        hasher.finalize().into()
    }
}

impl Transaction for DummyTransaction {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transactions<T: Transaction>(pub Vec<T>);

impl<T: Transaction, H: Proof> Block<T, H> {
    pub fn merkle_root(&self) -> Option<Vec<u8>> {
        self.txs.merkle_root()
    }

    pub fn new(prev: &Block<T, H>, txs: Transactions<T>, cfg: H::Config) -> Result<Block<T, H>> {
        let merkle_root = match txs.merkle_root() {
            Some(root) => root,
            None => bail!("No merkle root found!"),
        };
        let block = Block {
            header: BlockHeader {
                prev_hash: prev.header.hash().to_vec(),
                merkle_root,
                timestamp: Utc::now().timestamp(),
                proof: H::new(cfg),
            },
            txs,
        };
        Ok(block)
    }

    pub fn set_proof(&mut self, proof: H) {
        self.header.proof = proof;
    }

    pub fn validate(&self, prev: &Block<T, H>) -> bool {
        let prev_valid = self.header.prev_hash == prev.header.hash();
        debug!("prev_valid: {}", prev_valid);
        let time_valid = self.header.timestamp > prev.header.timestamp;
        debug!("time_valid: {}", time_valid);
        let merkle_valid = {
            let Some(calc) = self.merkle_root() else {
                return false;
            };
            calc == self.header.merkle_root
        };

        let proof_valid = self
            .header
            .proof
            .validate(&prev.header.proof, &self.header.hash());
        debug!("proof_valid: {}", proof_valid);

        prev_valid && time_valid && merkle_valid && proof_valid
    }

    pub fn genesis<TD: Transaction + Default>() -> Block<TD, H> {
        Block {
            header: BlockHeader {
                prev_hash: "0".repeat(64).as_bytes().to_vec(),
                merkle_root: "0".repeat(64).as_bytes().to_vec(),
                timestamp: 1685000000,
                proof: H::genesis_config(),
            },
            txs: Transactions::<TD>::test_new(),
        }
    }
}

impl<H: Proof> Hashable for BlockHeader<H> {
    fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.prev_hash.clone());
        hasher.update(self.merkle_root.clone());
        hasher.update(self.timestamp.to_le_bytes());
        hasher.update(self.proof.hash());
        let result = hasher.finalize();

        let mut hasher = Sha256::new();
        hasher.update(result);
        let result = hasher.finalize();
        result.into()
    }
}

impl<H: Proof + Display> Display for BlockHeader<H> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let s = format!(
            "
            block:\n\
            \ttimestamp: {}\n\
            \tproof field: {}\n\
        ",
            self.timestamp, self.proof
        );
        write!(f, "{}", s)
    }
}

impl<T: Transaction, H: Proof + Display> Display for Block<T, H> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.header)
    }
}

impl<T: Transaction + Default> Hashable for Transactions<T> {
    fn hash(&self) -> [u8; 32] {
        match self.merkle_root() {
            Some(root) => root.try_into().unwrap(),
            None => {
                // Default transaction
                let txs = Transactions(vec![T::default()]);
                txs.merkle_root().unwrap().try_into().unwrap()
            }
        }
    }
}

impl<T: Transaction + Default> Transaction for Transactions<T> {
    fn verify(&self) -> bool {
        self.0.iter().all(|tx| tx.verify())
    }
}

impl<T: Transaction> Transactions<T> {
    pub fn test_new<H: Transaction + Default>() -> Transactions<H> {
        Transactions(vec![H::default()])
    }

    pub fn merkle_root(&self) -> Option<Vec<u8>> {
        let leaves: Vec<[u8; 32]> = self.0.iter().map(|tx| tx.hash()).collect();
        let mt: MerkleTree<MerkleSha256> = MerkleTree::from_leaves(&leaves);
        let root = mt.root();
        root.map(|x| x.to_vec())
    }
}
