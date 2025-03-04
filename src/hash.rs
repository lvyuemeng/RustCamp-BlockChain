use sha2::{Digest, Sha256};
pub trait Hashable {
    fn hash(&self) -> [u8; 32];
}
