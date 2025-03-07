use serde::{Deserialize, Serialize};

use crate::{block::Transaction, hash::Hashable};

pub trait TransactionSign: Transaction {
    fn signer(&self) -> &str;
    fn signature(&self) -> &[u8];
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub enum TransactionType {
    Transfer { to: String, amount: u64 },
    Stake { amount: u64 },
}

#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct PoSTransaction {
    pub tx_type: TransactionType,
    pub signer: String,
    pub signature: Vec<u8>,
    pub sequence:u64,
}

impl Hashable for PoSTransaction {}

impl Transaction for PoSTransaction {}

impl TransactionSign for PoSTransaction {
    fn signature(&self) -> &[u8] {
        &self.signature
    }
    
    fn signer(&self) -> &str {
        &self.signer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct POS;
