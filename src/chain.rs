use anyhow::{Result, bail};
use num_bigint::BigUint;
use rocksdb::{DB, Direction, IteratorMode, Options, WriteBatch};
use serde::{Deserialize, Serialize};

use crate::{
    block::{Block, ProofWork},
    hash::{Hashable, target_to_bits},
    transaction::{DummyTransaction, Transaction},
};

// 2min span
const TARGET_TIME_SPAN: u64 = 120;
// 10 blocks adjust
const DIFFICULTY_ADJUST_INTERVAL: u64 = 10;
pub const DEFAULT_DIFFICULTY: u32 = 0x1d00_ffff;
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

impl BlockChain {
    pub fn new(path: &str) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
        opts.set_max_open_files(512);

        let db = DB::open(&opts, path)?;

        if db.get(b"last_hash")?.is_none() {
            let genesis = Block::<DummyTransaction>::genesis();
            let hash = genesis.header.hash();

            let mut batch = WriteBatch::default();
            batch.put(b"last_hash", &hash);
            batch.put(
                format!("block_{}", hex::encode(&hash)),
                bincode::serialize(&genesis)?,
            );
            batch.put(b"height_0", &hash);
            batch.put(b"height", &0u64.to_le_bytes());
            batch.put(b"cur_bits", &genesis.header.bits.to_le_bytes());
            db.write(batch)?;
        }

        let cur_bits = db
            .get(b"cur_bits")?
            .map(|v| u32::from_le_bytes(v[..4].try_into().unwrap()))
            .unwrap_or(DEFAULT_DIFFICULTY);
        Ok(Self { db, cur_bits })
    }

    pub fn add_block<T: Transaction + Serialize>(&self, block: Block<T>) -> Result<()> {
        let mut batch = WriteBatch::default();
        let block_hash = block.header.hash();

        batch.put(
            format!("block_{}", hex::encode(&block_hash)),
            bincode::serialize(&block)?,
        );
        batch.put(b"last_hash", &block_hash);

        let last_height = self.get_height()?;
        batch.put(format!("height_{}", last_height + 1), &block_hash);
        self.db.write(batch)?;
        Ok(())
    }

    pub fn get_block<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
        height: u64,
    ) -> Result<Block<T>> {
        let Some(block_hash) = self.db.get(format!("height_{}", height))? else {
            bail!("Block hash not found in given height!");
        };
        let Some(block_raw) = &self.db.get(format!("block_{}", hex::encode(&block_hash)))? else {
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
        let Some(block_raw) = &self.db.get(format!("block_{}", hex::encode(&block_hash)))? else {
            bail!("Block not found in given hash!");
        };

        let block = bincode::deserialize(block_raw)?;
        Ok(block)
    }

    pub fn get_difficulty<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &mut self,
    ) -> Result<u32> {
        let height = self.get_height()?;
        // height >= 10.
        if height % DIFFICULTY_ADJUST_INTERVAL != 0 {
            return Ok(self.cur_bits);
        }
        let first_block: Block<T> = self.get_block(height - DIFFICULTY_ADJUST_INTERVAL)?;
        let last_block: Block<T> = self.get_last_block()?;

        let actual_span = last_block.header.timestamp - first_block.header.timestamp;
        // avoid divide zero
        let actual_span = actual_span.max(1);

        let prev_target = ProofWork::from_bits(first_block.header.bits).target();
        let mut new_target = prev_target.clone() * TARGET_TIME_SPAN / actual_span as u64;

        let max_target = prev_target.clone() * BigUint::from(4u32);
        let min_target = prev_target.clone() / BigUint::from(4u32);
        new_target = new_target.clamp(min_target, max_target);

        let new_bits = target_to_bits(new_target);

        self.db.put(b"cur_bits", &new_bits.to_le_bytes())?;
        self.cur_bits = new_bits;
        Ok(new_bits)
    }

    fn get_height(&self) -> Result<u64> {
        let Some(height) = self
            .db
            .get(b"height")
            .map(|v| v.map(|b| u64::from_le_bytes(b[..8].try_into().unwrap())))?
        else {
            return Ok(0);
        };

        Ok(height)
    }
}
