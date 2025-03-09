pub mod pow;

use std::path::Path;

use anyhow::{Result, bail};
use rocksdb::{DB, Options, WriteBatch};
use serde::Deserialize;

use crate::{
    block::{Block, Consensus, Transaction, Transactions},
    hash::Hashable,
};

pub mod blockchain_control {
    pub const TARGET_TIME_SPAN: u64 = 120;
    pub const DIFFICULTY_ADJUST_INTERVAL: u64 = 10;
    pub const DEFAULT_DIFFICULTY: u32 = 0x1f00_ffff;
}

pub struct DbKeys;

impl DbKeys {
    pub const LAST_HASH: &'static [u8] = b"last_hash";
    pub const CUR_HEIGHT: &'static [u8] = b"height";
    pub const CUR_STATE: &'static [u8] = b"state";

    pub fn block_key(hash: &[u8]) -> Vec<u8> {
        format!("block_{}", hex::encode(hash)).into_bytes()
    }

    pub fn hash_from_block_key(key: &[u8]) -> Option<Vec<u8>> {
        key.strip_prefix(b"block_").map(|s| s.to_vec())
    }

    pub fn height_key(height: u64) -> Vec<u8> {
        format!("height_{:016x}", height).into_bytes()
    }
}

pub struct BlockChain<C: Consensus> {
    db: DB,
    cs: C,
}

impl<C: Consensus + for<'a> Deserialize<'a>> BlockChain<C> {
    pub fn new<T: Transaction + Default>(path: impl AsRef<Path>) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
        opts.set_max_open_files(512);

        log::info!("Opened db at {:?}", path.as_ref().display());
        let db = DB::open(&opts, path)?;

        let cur_state = match db.get(DbKeys::CUR_STATE)? {
            Some(state) => bincode::deserialize(&state)?,
            None => C::default(),
        };

        if db.get(DbKeys::height_key(0))?.is_none() {
            log::info!("No last hash, Creating genesis block");
            let genesis: Block<T, C> = Block::<T, C>::genesis();
            let hash = genesis.header.hash();
            
            let mut batch = WriteBatch::default();
            batch.put(DbKeys::block_key(&hash), bincode::serialize(&genesis)?);
            batch.put(DbKeys::LAST_HASH, &hash);
            batch.put(DbKeys::height_key(0), &hash);
            batch.put(DbKeys::CUR_HEIGHT, &0u64.to_le_bytes());
            batch.put(DbKeys::CUR_STATE, bincode::serialize(&cur_state)?);
            db.write(batch)?;
        }

        Ok(Self { db, cs: cur_state })
    }

    pub fn get_consensus(&self) -> &C {
        &self.cs
    }

    pub fn get_consensus_mut(&mut self) -> &mut C {
        &mut self.cs
    }

    pub fn add_block<
        T: Transaction + for<'a> Deserialize<'a>,
    >(
        &mut self,
        block: Block<T, C>,
    ) -> Result<()> {
        self.validate_new(&block)?;
        let mut batch = WriteBatch::default();
        let block_hash = block.header.hash();

        batch.put(DbKeys::block_key(&block_hash), bincode::serialize(&block)?);
        batch.put(DbKeys::LAST_HASH, &block_hash);

        let new_height = self.get_height()? + 1;
        batch.put(DbKeys::height_key(new_height), &block_hash);
        batch.put(DbKeys::CUR_HEIGHT, &new_height.to_le_bytes());

        self.db.write(batch)?;
        Ok(())
    }

    fn validate_new<
        T: Transaction + for<'a> Deserialize<'a>,
    >(
        &self,
        block: &Block<T, C>,
    ) -> Result<()> {
        if block.validate(&self.get_last_block()?) {
            Ok(())
        } else {
            bail!("Invalid block!");
        }
    }

    pub fn get_block<
        T: Transaction + for<'a> Deserialize<'a>,
    >(
        &self,
        height: u64,
    ) -> Result<Block<T, C>> {
        let block_hash = self
            .db
            .get(DbKeys::height_key(height))?
            .ok_or_else(|| anyhow::anyhow!("Block hash not found at height {}", height))?;
        let block_raw = self
            .db
            .get(DbKeys::block_key(&block_hash))?
            .ok_or_else(|| anyhow::anyhow!("Block not found for given hash!"))?;
        Ok(bincode::deserialize(&block_raw)?)
    }

    pub fn get_last_block<
        T: Transaction + for<'a> Deserialize<'a>,
    >(
        &self,
    ) -> Result<Block<T, C>> {
        self.get_block(self.get_height()?)
    }

    pub fn get_height(&self) -> Result<u64> {
        self.db
            .get(DbKeys::CUR_HEIGHT)?
            .map(|v| u64::from_le_bytes(v[..8].try_into().unwrap()))
            .or_else(|| Some(0))
            .ok_or_else(|| anyhow::anyhow!("Blockchain height not found"))
    }

    pub fn put_state(&self, chain: &BlockChain<C>) -> Result<()> {
        chain
            .db
            .put(DbKeys::CUR_STATE, bincode::serialize(&self.cs)?)
            .map_err(|e| anyhow::anyhow!(e))
    }

    pub fn get_state(&self) -> Result<C::Data> {
        let state = self.db.get(DbKeys::CUR_STATE)?;
        match state {
            Some(s) => {
                let s = bincode::deserialize(&s).map_err(|e| anyhow::anyhow!(e))?;
                Ok(s)
            }
            None => {
                bail!("Can't found state for consensus!")
            }
        }
    }
}
