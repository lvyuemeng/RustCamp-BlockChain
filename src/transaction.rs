use crate::hash::Hashable;
use serde::{Deserialize, Serialize};
use sha2::{Digest,Sha256};

pub trait Transaction: Hashable {}

#[derive(Debug,Serialize,Deserialize)]
pub struct DummyTransaction;

impl Hashable for DummyTransaction {
	fn hash(&self) -> [u8; 32] {
		let mut hasher = Sha256::new();
		hasher.update(b"Dummy");
		hasher.finalize().into()
	}
}

impl Transaction for DummyTransaction {}