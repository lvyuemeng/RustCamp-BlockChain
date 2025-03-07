pub mod consensus;
pub mod pow;

use std::path::Path;

use anyhow::{Result, bail};
use consensus::{Consensus, PoWConfig};
use rocksdb::{DB, Options, WriteBatch};
use serde::{Deserialize, Serialize};

use crate::{
    block::{Block, DummyTransaction, Proof, Transaction},
    hash::Hashable,
};

pub mod blockchain_control {
    pub const TARGET_TIME_SPAN: u64 = 120;
    pub const DIFFICULTY_ADJUST_INTERVAL: u64 = 10;
    pub const DEFAULT_DIFFICULTY: u32 = 0x1f00_ffff;
}

struct DbKeys;

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

pub struct BlockChain<C> {
    db: DB,
    cs: Consensus<C>,
}

impl<C: Serialize + for<'a> Deserialize<'a> + Default> BlockChain<C> {
    pub fn new<T: Transaction + Default, P: Proof>(path: impl AsRef<Path>) -> Result<Self> {
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
            let genesis: Block<T, P> = Block::<T, P>::genesis();
            let hash = genesis.header.hash();

            let mut batch = WriteBatch::default();
            batch.put(DbKeys::block_key(&hash), bincode::serialize(&genesis)?);
            batch.put(DbKeys::LAST_HASH, &hash);
            batch.put(DbKeys::height_key(0), &hash);
            batch.put(DbKeys::CUR_HEIGHT, &0u64.to_le_bytes());
            batch.put(DbKeys::CUR_STATE, bincode::serialize(&cur_state)?);
            db.write(batch)?;
        }

        Ok(Self {
            db,
            cs: Consensus::new(cur_state),
        })
    }

    pub fn add_block<
        T: Transaction + for<'a> Deserialize<'a>,
        P: Proof + for<'a> Deserialize<'a>,
    >(
        &self,
        block: Block<T, P>,
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
        P: Proof + for<'a> Deserialize<'a>,
    >(
        &self,
        block: &Block<T, P>,
    ) -> Result<()> {
        if block.validate(&self.get_last_block()?) {
            Ok(())
        } else {
            bail!("Invalid block!");
        }
    }

    pub fn get_block<
        T: Transaction + for<'a> Deserialize<'a>,
        P: Proof + for<'a> Deserialize<'a>,
    >(
        &self,
        height: u64,
    ) -> Result<Block<T, P>> {
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
        P: Proof + for<'a> Deserialize<'a>,
    >(
        &self,
    ) -> Result<Block<T, P>> {
        self.get_block(self.get_height()?)
    }

    pub fn get_height(&self) -> Result<u64> {
        self.db
            .get(DbKeys::CUR_HEIGHT)?
            .map(|v| u64::from_le_bytes(v[..8].try_into().unwrap()))
            .or_else(|| Some(0))
            .ok_or_else(|| anyhow::anyhow!("Blockchain height not found"))
    }
}
