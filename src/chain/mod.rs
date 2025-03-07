use std::path::Path;

use anyhow::{Result, bail};
use rocksdb::{DB, Options, WriteBatch};
use serde::{Deserialize, Serialize};

use crate::{
    block::{Block, Proof, Transaction},
    hash::{Hashable, target_to_bits},
};

pub mod blockchain_control {
    pub const TARGET_TIME_SPAN: u64 = 120;
    pub const DIFFICULTY_ADJUST_INTERVAL: u64 = 10;
    pub const DEFAULT_DIFFICULTY: u32 = 0x1f00_ffff;
}

pub mod consensus {
    use serde::{Deserialize, Serialize};

    use crate::block::{pow::POW, Block, Proof, Transaction};
    use crate::chain::{blockchain_control, BlockChain};
    use crate::hash::target_to_bits;


    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PoWConfig {
        pub target_timespan: u64,
        pub difficulty_adjust_interval: u64,
        pub initial_difficulty: u32,
        pub allow_mining_reward: bool,
        pub block_reward: u64,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct PoSConfig {
        pub min_stake_amount: u64,
        pub stake_lock_period: u64, // pledge blocks
        pub annual_interest_rate: f64,
        pub validator_count: usize,
        pub epoch_length: u64,
        pub security_deposit: u64,
    }
    
    pub struct Consensus<C,H:Proof> {
        pub config:C,
        _mark:std::marker::PhantomData<H>
    }
    
    impl<C,H:Proof> Consensus<C,H> {
        fn new(config: C) -> Self {
            Self { config, _mark: std::marker::PhantomData }
        }
    }

    impl Consensus<PoWConfig, POW> {
        fn adjust_difficulty<T:Transaction>(&self, chain: &BlockChain<PoWConfig,POW>) -> u32 {
        let height = chain.get_height()?;
        if height % self.config.initial_difficulty != 0 || height == 0 {
            // TODO!
            return Ok(self.);
        }
        let first_block: Block<T,POW> =
            self.get_block(height - blockchain_control::DIFFICULTY_ADJUST_INTERVAL)?;
        let last_block: Block<T,POW> = self.get_last_block()?;

        let actual_span = (last_block.header.timestamp - first_block.header.timestamp).max(1);
        let prev_target = POW::from_bits(first_block.header.proof.bits);
        let new_target =
            prev_target.clone() * blockchain_control::TARGET_TIME_SPAN / actual_span as u64;
        let new_target = new_target.clamp(prev_target.clone() / 4u32, prev_target.clone() * 4u32);
        let new_bits = target_to_bits(new_target);

        self.db.put(DbKeys::CUR_BITS, &new_bits.to_le_bytes())?;
        self.cur_bits = new_bits;
        Ok(new_bits)
        }
    }
    
    impl Default for Consensus<PoWConfig, POW> {
        fn default() -> Self {
            Self {
                config: PoWConfig {
                    target_timespan: blockchain_control::TARGET_TIME_SPAN,
                    difficulty_adjust_interval: blockchain_control::DIFFICULTY_ADJUST_INTERVAL,
                    initial_difficulty: blockchain_control::DEFAULT_DIFFICULTY,
                    allow_mining_reward: true,
                    block_reward: 50,
                },
                _mark: std::marker::PhantomData
            }
        }
    }
    
    impl Consensus<PoSConfig, POS> {} 
}

struct DbKeys;

impl DbKeys {
    const LAST_HASH: &'static [u8] = b"last_hash";
    const CUR_HEIGHT: &'static [u8] = b"height";
    const CUR_BITS: &'static [u8] = b"cur_bits";

    fn block_key(hash: &[u8]) -> Vec<u8> {
        format!("block_{}", hex::encode(hash)).into_bytes()
    }

    fn hash_from_block_key(key: &[u8]) -> Option<Vec<u8>> {
        key.strip_prefix(b"block_").map(|s| s.to_vec())
    }

    fn height_key(height: u64) -> Vec<u8> {
        format!("height_{:016x}", height).into_bytes()
    }
}

pub struct BlockChain<CC,H: Proof> {
    db: DB,
    cs: consensus::Consensus<CC,H>
}

impl BlockChain {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::DBCompressionType::Zstd);
        opts.set_max_open_files(512);

        log::info!("Opened db at {:?}", path.as_ref().display());
        let db = DB::open(&opts, path)?;

        let cur_bits = match db.get(DbKeys::CUR_BITS)? {
            Some(bits) => u32::from_le_bytes(bits[..4].try_into().unwrap()),
            None => blockchain_control::DEFAULT_DIFFICULTY,
        };

        if db.get(DbKeys::height_key(0))?.is_none() {
            log::info!("No last hash, Creating genesis block");
            let genesis: Block<DummyTransaction> = Block::<DummyTransaction>::genesis();
            let hash = genesis.header.hash();

            let mut batch = WriteBatch::default();
            batch.put(DbKeys::block_key(&hash), bincode::serialize(&genesis)?);
            batch.put(DbKeys::LAST_HASH, &hash);
            batch.put(DbKeys::height_key(0), &hash);
            batch.put(DbKeys::CUR_HEIGHT, &0u64.to_le_bytes());
            batch.put(DbKeys::CUR_BITS, &genesis.header.bits.to_le_bytes());
            db.write(batch)?;
        }
        log::info!("Current difficulty: {}", cur_bits);
        Ok(Self { db, cur_bits })
    }

    pub fn add_block<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
        block: Block<T>,
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

    fn validate_new<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
        block: &Block<T>,
    ) -> Result<()> {
        if block.validate(&self.get_last_block()?) {
            Ok(())
        } else {
            bail!("Invalid block!");
        }
    }

    pub fn get_block<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
        height: u64,
    ) -> Result<Block<T>> {
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

    pub fn get_last_block<T: Transaction + Serialize + for<'a> Deserialize<'a>>(
        &self,
    ) -> Result<Block<T>> {
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
