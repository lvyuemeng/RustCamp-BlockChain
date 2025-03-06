use crate::hash::Hashable;
use rs_merkle::{MerkleTree, algorithms::Sha256 as MerkleSha256};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub trait Transaction: Hashable {
    fn validate(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transactions<T: Transaction>(pub Vec<T>);

impl<T:Transaction> Hashable for Transactions<T> {
    fn hash(&self) -> [u8; 32] {
		match self.merkle_root() {
			Some(root) => {
				root.try_into().unwrap()
			}
			None => {
				let mut hasher = Sha256::new();
				hasher.update(b"Txs");
				hasher.finalize().into()
			}
		}
    }
}

impl<T: Transaction> Transactions<T> {
	pub fn test_new<H:Transaction + Default>() -> Transactions<H> {
		Transactions(vec![H::default()])
	}
    pub fn validate(&self) -> bool {
        self.0.iter().all(|tx| tx.validate())
    }

    pub fn merkle_root(&self) -> Option<Vec<u8>> {
        let leaves: Vec<[u8; 32]> = self.0.iter().map(|tx| tx.hash()).collect();
        let mt: MerkleTree<MerkleSha256> = MerkleTree::from_leaves(&leaves);
        let root = mt.root();
        root.map(|x| x.to_vec())
    }
}

#[derive(Debug, Serialize, Deserialize,Default)]
pub struct DummyTransaction;

impl Hashable for DummyTransaction {
    fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"Dummy");
        hasher.finalize().into()
    }
}

impl Transaction for DummyTransaction {}
