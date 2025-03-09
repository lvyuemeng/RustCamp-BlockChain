use std::{collections::HashMap, fmt::Display};

use anyhow::{Result, bail};
use chrono::Utc;
use ed25519_dalek::{SecretKey, Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{block::Transaction, hash::Hashable};

use super::{Block, BlockHeader, Consensus, Transactions};

pub trait TransactionSign: Transaction {
    fn signer(&self) -> &str;
    fn signature(&self) -> &[u8];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionType {
    Transfer { to: String, amount: u64 },
    Stake { amount: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoSTransaction {
    pub tx_type: TransactionType,
    pub signer: String,
    pub signature: Vec<u8>,
    pub sequence: u64,
}

impl Default for PoSTransaction {
    fn default() -> Self {
        PoSTransaction {
            tx_type: TransactionType::Transfer {
                to: "".to_string(),
                amount: 0,
            },
            signer: "".to_string(),
            signature: vec![],
            sequence: 0,
        }
    }
}

impl Hashable for PoSTransaction {
    fn try_hash(&self) -> Option<[u8; 32]> {
        let mut hasher = Sha256::new();
        let Some(val) = bincode::serialize(&self).ok() else {
            return None;
        };
        hasher.update(val);
        Some(hasher.finalize().into())
    }
}

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
pub struct PoS {
    #[serde(skip)]
    pub min_stake_amount: u64,
    #[serde(skip)]
    pub stake_lock_period: u64, // pledge blocks
    #[serde(skip)]
    pub annual_interest_rate: f64,
    #[serde(skip)]
    pub validator_count: usize,
    #[serde(skip)]
    pub epoch_length: u64,
    #[serde(skip)]
    pub security_deposit: u64,

    pub cur_validators: HashMap<VerifyingKey, u64>,
    // Insecure! For demonstration only
    #[serde(skip)]
    pub validator_keys: HashMap<VerifyingKey, SecretKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoSData {
    pub validator_key: VerifyingKey,
    pub signature: Signature,
}

impl Display for PoSData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PoS\n validator: {:?}\n signature: {:?}",
            self.validator_key, self.signature,
        )
    }
}

impl Hashable for PoS {
    fn try_hash(&self) -> Option<[u8; 32]> {
        let mut hasher = Sha256::new();
        let val = bincode::serialize(&self).unwrap();
        hasher.update(val);
        Some(hasher.finalize().into())
    }
}

impl Default for PoS {
    fn default() -> Self {
        Self {
            min_stake_amount: 1000,
            stake_lock_period: 10000,
            annual_interest_rate: 0.1,
            validator_count: 5,
            epoch_length: 100,
            security_deposit: 100,
            cur_validators: HashMap::new(),
            validator_keys: HashMap::new(),
        }
    }
}

impl PoS {
    pub fn add_validator(&mut self, secret_key: SecretKey, stake: u64) {
        let public_key = SigningKey::from_bytes(&secret_key).verifying_key();

        self.cur_validators.insert(public_key, stake);
        self.validator_keys.insert(public_key, secret_key);
    }

    fn select_validator(&self) -> Option<VerifyingKey> {
        let total_stake: u64 = self.cur_validators.values().sum();
        if total_stake == 0 {
            return None;
        }

        let mut rng = rand::rng();
        let mut random = rng.random_range(0..total_stake);

        for (pub_key, &stake) in &self.cur_validators {
            if random < stake {
                return Some(*pub_key);
            }
            random -= stake;
        }

        None
    }
}

impl Consensus for PoS {
    type Data = PoSData;

    fn validate<T: Transaction>(&self, block: &Block<T, Self>) -> bool {
        let pub_key = block.header.data.validator_key.clone();
        let signature = block.header.data.signature.clone();

        // Check validator has sufficient stake in previous state
        let has_stake = &self
            .cur_validators
            .get(&pub_key)
            .map_or(false, |&stake| stake >= self.min_stake_amount);

        pub_key.verify(&block.header.hash(), &signature).is_ok() && *has_stake
    }

    fn generate_block<T: Transaction>(
        &self,
        block: &Block<T, Self>,
        txs: Transactions<T>,
    ) -> Result<Block<T, Self>> {
        let Some(validator_pubkey) = self.select_validator() else {
            bail!("No validator selected");
        };
        let Some(secret_key) = self.validator_keys.get(&validator_pubkey).cloned() else {
            bail!("No secret key found");
        };

        let hash = block.header.hash();
        let signature = SigningKey::from_bytes(&secret_key).sign(&hash);

        let Some(merkle_root) = txs.merkle_root() else {
            bail!("No merkle root found");
        };

        let block = Block {
            header: BlockHeader {
                prev_hash: block.header.hash().to_vec(),
                merkle_root,
                timestamp: Utc::now().timestamp(),
                data: PoSData {
                    validator_key: validator_pubkey,
                    signature,
                },
            },
            txs,
        };

        Ok(block)
    }

    fn genesis_data() -> Self::Data {
        PoSData {
            validator_key: VerifyingKey::from_bytes(&[0; 32]).unwrap(),
            signature: Signature::from_bytes(&[0; 64]),
        }
    }
}
