use anyhow::{Result, bail};
use num_bigint::BigUint;
use rocksdb::{DB, Direction, IteratorMode, Options, WriteBatch};
use serde::{Deserialize, Serialize};

use crate::{
    block::{Block, ProofWork},
    hash::{Hashable, target_to_bits},
    transaction::{DummyTransaction, Transaction},
};

pub mod blockchain_control {
    // 2min span
    pub const TARGET_TIME_SPAN: u64 = 120;
    // 10 blocks adjust
    pub const DIFFICULTY_ADJUST_INTERVAL: u64 = 10;
    pub const DEFAULT_DIFFICULTY: u32 = 0x1d00_ffff;
}

/// ## BlockChain
///
/// DB storage layout:
///
/// - key: block_{block_hash} val: block data
///
/// - key: height_{height} val: block_hash
///
/// - key: last_hash val: block_hash
///
/// - key: cur_bits val: bits
///
/// - key: height val: height
pub struct BlockChain {
    db: DB,
    cur_bits: u32,
}

mod db_keys {
    use serde::Serialize;

    use crate::{block::Block, hash::Hashable, transaction::Transaction};

    pub const LAST_HASH: &[u8] = b"last_hash";
    pub const HEIGHT: &[u8] = b"height";
    pub const CUR_BITS: &[u8] = b"cur_bits";

    pub fn block_key<T: Transaction + Serialize>(block: &Block<T>) -> Vec<u8> {
        format!("block_{}", block.header.hash_string()).into_bytes()
    }

    pub fn block_key_from_hash(hash: &[u8]) -> Vec<u8> {
        format!("block_{}", hex::encode(hash)).into_bytes()
    }

    pub fn height_key(height: u64) -> Vec<u8> {
        format!("height_{}", height).into_bytes()
    }

    // pub fn parse_height_key(key: &[u8]) -> Option<u64> {
    //     let prefix = b"height_";
    //     key.strip_prefix(prefix)
    //         .and_then(|s| std::str::from_utf8(s).ok())
    //         .and_then(|s| s.parse().ok())
    // }
}

impl BlockChain {
    pub fn new(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
        opts.set_max_open_files(512);

        let db = DB::open(&opts, path)?;

        if db.get(db_keys::LAST_HASH)?.is_none() {
            let genesis = Block::<DummyTransaction>::genesis();
            let hash = genesis.header.hash();

            let mut batch = WriteBatch::default();
            batch.put(db_keys::LAST_HASH, &hash);
            batch.put(db_keys::block_key(&genesis), bincode::serialize(&genesis)?);
            batch.put(db_keys::height_key(0), &hash);
            batch.put(db_keys::HEIGHT, &0u64.to_le_bytes());
            batch.put(db_keys::CUR_BITS, &genesis.header.bits.to_le_bytes());
            db.write(batch)?;
        }

        let cur_bits = db
            .get(db_keys::CUR_BITS)?
            .map(|v| u32::from_le_bytes(v[..4].try_into().unwrap()))
            .unwrap_or(blockchain_control::DEFAULT_DIFFICULTY);
        Ok(Self { db, cur_bits })
    }

    pub fn add_block<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
        block: Block<T>,
    ) -> Result<()> {
        self.validate_new(&block)?;
        let mut batch = WriteBatch::default();
        let block_hash = block.header.hash();

        batch.put(db_keys::block_key(&block), bincode::serialize(&block)?);
        batch.put(db_keys::LAST_HASH, &block_hash);

        let last_height = self.get_height()?;
        batch.put(db_keys::height_key(last_height + 1), &block_hash);
        self.db.write(batch)?;
        Ok(())
    }

    fn validate_new<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
        block: &Block<T>,
    ) -> Result<()> {
        let last_block = self.get_last_block()?;
        if block.validate(&last_block) {
            Ok(())
        } else {
            bail!("Invalid block!");
        }
    }

    pub fn get_block<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
        height: u64,
    ) -> Result<Block<T>> {
        let Some(block_hash) = self.db.get(db_keys::height_key(height))? else {
            bail!("Block hash not found in given height!");
        };
        let Some(block_raw) = &self.db.get(db_keys::block_key_from_hash(&block_hash))? else {
            bail!("Block not found in given hash!");
        };

        let block = bincode::deserialize(block_raw)?;
        Ok(block)
    }

    pub fn get_last_block<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
    ) -> Result<Block<T>> {
        let mut iter = self
            .db
            .iterator(IteratorMode::From(b"height_", Direction::Reverse));
        let Some(Ok((_, block_hash))) = iter.next() else {
            bail!("No block in blockchain!");
        };
        let Some(block_raw) = &self.db.get(db_keys::block_key_from_hash(&block_hash))? else {
            bail!("Block not found in given hash!");
        };

        let block = bincode::deserialize(block_raw)?;
        Ok(block)
    }

    pub fn get_difficulty<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &mut self,
    ) -> Result<u32> {
        let height  = self.get_height()?;
        
        if height % blockchain_control::DIFFICULTY_ADJUST_INTERVAL != 0 {
            return Ok(self.cur_bits);
        }
        // safety: height <= interval is excluded by % interval operaiton.
        let first_block: Block<T> =
            self.get_block(height - blockchain_control::DIFFICULTY_ADJUST_INTERVAL)?;
        let last_block: Block<T> = self.get_last_block()?;

        // avoid divide zero
        let actual_span = (last_block.header.timestamp - first_block.header.timestamp).max(1);

        let prev_target = ProofWork::from_bits(first_block.header.bits).target();
        let mut new_target =
            prev_target.clone() * blockchain_control::TARGET_TIME_SPAN / actual_span as u64;

        let max_target = prev_target.clone() * BigUint::from(4u32);
        let min_target = prev_target.clone() / BigUint::from(4u32);
        new_target = new_target.clamp(min_target, max_target);

        let new_bits = target_to_bits(new_target);

        self.db.put(db_keys::CUR_BITS, &new_bits.to_le_bytes())?;
        self.cur_bits = new_bits;
        Ok(new_bits)
    }

    fn get_height(&self) -> Result<u64> {
        let Some(height) = self
            .db
            .get(db_keys::HEIGHT)
            .map(|v| v.map(|b| u64::from_le_bytes(b[..8].try_into().unwrap())))?
        else {
            // assert: Must exist genesis
            return Ok(0);
        };

        Ok(height)
    }
}
