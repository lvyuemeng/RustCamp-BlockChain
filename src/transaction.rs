use crate::hash::Hashable;
use sha2::{Digest,Sha256};

pub trait Transaction: Hashable {}
