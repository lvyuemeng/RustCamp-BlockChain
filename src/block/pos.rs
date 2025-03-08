use std::{collections::HashMap, default};

use anyhow::Result;
use ed25519_dalek::{
    PUBLIC_KEY_LENGTH, SecretKey, Signature, Signer, SigningKey, Verifier, VerifyingKey,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{block::Transaction, chain::consensus::PoSConfig, hash::Hashable};

use super::{Block, Proof, Transactions};

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
    fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(bincode::serialize(self).unwrap());
        hasher.finalize().into()
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
    validator: Vec<u8>,
    #[serde(skip)]
    signature: Vec<u8>,
    #[serde(skip)]
    stakes: HashMap<Vec<u8>, u64>,
    #[serde(skip)]
    validator_keys: HashMap<Vec<u8>, SecretKey>,
}

impl Hashable for PoS {
    fn hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(bincode::serialize(self).unwrap());
        hasher.finalize().into()
    }
}

impl PoS {
    pub fn generate_block(
        &self,
        prev_block: &Block<PoSTransaction, PoS>,
        txs: Transactions<PoSTransaction>,
    ) -> Result<Block<PoSTransaction, PoS>> {
        let validator_pubkey = self.select_validator().expect("No validators available");
        let secret_key = self.validator_keys.get(&validator_pubkey).unwrap().clone();

        let mut block = Block::new(
            prev_block,
            txs,
            PoSConfig {
                validator: validator_pubkey.clone(),
                ..Default::default()
            },
        )?;

        let hash = block.header.hash();

        let signature = SigningKey::from_bytes(&secret_key).sign(&hash);
        block.set_proof(PoS {
            validator: validator_pubkey,
            signature: signature.to_bytes().to_vec(),
            stakes: self.stakes.clone(),
            validator_keys: self.validator_keys.clone(),
        });

        Ok(block)
    }

    pub fn add_validator(&mut self, secret_key: SecretKey, stake: u64) {
        let public_key = SigningKey::from_bytes(&secret_key).verifying_key();
        self.stakes.insert(public_key.to_bytes().to_vec(), stake);
        self.validator_keys
            .insert(public_key.to_bytes().to_vec(), secret_key);
    }

    fn select_validator(&self) -> Option<Vec<u8>> {
        let total_stake: u64 = self.stakes.values().sum();
        if total_stake == 0 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let mut random = rng.gen_range(0..total_stake);

        for (pub_key, &stake) in &self.stakes {
            if random < stake {
                return Some(pub_key.clone());
            }
            random -= stake;
        }

        None
    }
}

impl Proof for PoS {
    type Config = PoSConfig;

    fn validate(&self, _prev: &Self, header_hash: &[u8]) -> bool {
        let mut key = [0u8; PUBLIC_KEY_LENGTH];
        key.clone_from_slice(&self.validator);
        let pub_key = VerifyingKey::from_bytes(&key).expect("invalid public key");
        let signature = Signature::from_slice(&self.signature).expect("invalid signature");

        pub_key.verify(header_hash, &signature).is_ok() && self.stakes.contains_key(&self.validator)
    }

    fn genesis_config() -> Self {
        Self {
            validator: vec![],
            signature: vec![],
            stakes: HashMap::new(),
            validator_keys: HashMap::new(),
        }
    }

    fn new(ctx: Self::Config) -> Self {
        Self {
            validator: ctx.validator,
            signature: ctx.signature,
            stakes: HashMap::new(),
            validator_keys: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{block::Block, chain::BlockChain};
    use ed25519_dalek::SigningKey;
    use rand_core::OsRng;
    use std::env::temp_dir;

    fn test_db<C: Serialize + for<'a> Deserialize<'a> + Default>() -> BlockChain<C> {
        let random_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let db_dir = temp_dir().join(format!("blockchain_test_{}", random_suffix));

        std::fs::create_dir_all(&db_dir).unwrap();
        let chain = BlockChain::new::<PoSTransaction, PoS>(db_dir).unwrap();

        chain
    }

    #[test]
    fn test_pos() {
        // 使用 PoS 的区块链
        let mut csprng = OsRng;
        let secret_key = SigningKey::generate(&mut csprng);
        let mut pos_consensus = PoS::new(PoSConfig {
            validator: secret_key.verifying_key().as_bytes().to_vec(),
            ..Default::default()
        });
        println!(
            "Added validators: {:?}: {}",
            secret_key.verifying_key().as_bytes(),
            60
        );
        pos_consensus.add_validator(secret_key.to_bytes(), 60);
        let secret_key = SigningKey::generate(&mut csprng);
        println!(
            "Added validators: {:?}: {}",
            secret_key.verifying_key().as_bytes(),
            100
        );
        pos_consensus.add_validator(secret_key.to_bytes(), 100);
        let secret_key = SigningKey::generate(&mut csprng);
        println!(
            "Added validators: {:?}: {}",
            secret_key.verifying_key().as_bytes(),
            80
        );
        pos_consensus.add_validator(secret_key.to_bytes(), 80);

        let mut pos_chain = test_db::<PoSConfig>();
        println!(
            "Genesis Block: {:?}",
            pos_chain.get_block::<PoSTransaction, PoS>(0)
        );

        let block = pos_consensus
            .generate_block(
                &pos_chain.get_last_block::<_, PoS>().unwrap(),
                Transactions(vec![PoSTransaction {
                    tx_type: TransactionType::Stake { amount: 50 },
                    ..Default::default()
                }]),
            )
            .unwrap();
        pos_chain.add_block(block).unwrap();
        assert_eq!(pos_chain.get_height().unwrap(), 1);

        let block = pos_consensus
            .generate_block(
                &pos_chain.get_last_block::<_, PoS>().unwrap(),
                Transactions(vec![PoSTransaction {
                    tx_type: TransactionType::Stake { amount: 20 },
                    ..Default::default()
                }]),
            )
            .unwrap();
        pos_chain.add_block(block).unwrap();

        let block = pos_consensus
            .generate_block(
                &pos_chain.get_last_block::<_, PoS>().unwrap(),
                Transactions(vec![PoSTransaction {
                    tx_type: TransactionType::Transfer {
                        to: "Alice".into(),
                        amount: 20,
                    },
                    ..Default::default()
                }]),
            )
            .unwrap();
        pos_chain.add_block(block).unwrap();

        let block = pos_consensus
            .generate_block(
                &pos_chain.get_last_block::<_, PoS>().unwrap(),
                Transactions(vec![PoSTransaction {
                    tx_type: TransactionType::Stake { amount: 50 },
                    ..Default::default()
                }]),
            )
            .unwrap();
        pos_chain.add_block(block).unwrap();

        println!("\n=========================== PoS Blockchain: =============================");
        for i in 0..pos_chain.get_height().unwrap() {
            let block: Block<PoSTransaction, PoS> = pos_chain.get_block(i).unwrap();
            println!("\nBlock {}: {:?}", i, block);
        }
    }
}
