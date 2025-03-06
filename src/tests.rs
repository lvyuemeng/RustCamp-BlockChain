#[cfg(test)]
mod tests {
    use std::{env::temp_dir, thread, time::Duration};

    use chrono::Utc;
    use serde::{Deserialize, Serialize};

    use crate::{
        block::{Block, BlockHeader, ProofWork},
        chain::{blockchain_control, BlockChain},
        hash::{bits_to_target, Hashable},
        transaction::{Transaction, Transactions},
    };

    const TEST_BITS: u32 = 0x1f00_ffff;

    #[derive(Debug, Serialize, Deserialize,Default)]
    pub struct TestTransaction;

    impl Hashable for TestTransaction {
        fn hash(&self) -> [u8; 32] {
            [0u8; 32]
        }
    }

    impl Transaction for TestTransaction {}

    fn log_init() {
        let _ = env_logger::builder().is_test(true).try_init();
    }

    fn test_new_block(chain: &mut BlockChain) -> Block<TestTransaction> {
        let prev :Block<TestTransaction>= chain.get_last_block().unwrap();
        let bits = chain.get_difficulty::<TestTransaction>().unwrap();
        log::debug!("bits: {:x}", bits);

        let txs = Transactions::<TestTransaction>::test_new();
        let merkle_root = txs.merkle_root().unwrap();
        let mut block = Block {
            header: BlockHeader {
                prev_hash: prev.header.hash().to_vec(),
                merkle_root,
                timestamp: Utc::now().timestamp(),
                bits,
                nonce: 0,
            },
            txs,
        };

        block.mine();
        block
    }

    fn test_db() -> BlockChain {
        let random_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let db_dir = temp_dir().join(format!("blockchain_test_{}", random_suffix));

        std::fs::create_dir_all(&db_dir).unwrap();
        let chain = BlockChain::new(db_dir).unwrap();

        chain
    }

    fn test_add(chain: &mut BlockChain) {
        let block = test_new_block(chain);
        chain.add_block(block).unwrap();
    }

    #[test]
    fn test_pow_validation() {
        let mut block: Block<TestTransaction> = Block::<TestTransaction>::genesis();
        block.header.bits = TEST_BITS;
        block.mine();

        let pow = ProofWork::test_bits(block.header.bits);
        assert!(pow.is_valid(&block.header.hash()))
    }

    #[test]
    fn test_bits_target_transform() {
        let bits = blockchain_control::DEFAULT_DIFFICULTY;
        let target = bits_to_target(bits);

        log::info!("target: {}", target);
    }

    #[test]
    fn test_blockchain_creation() {
        let chain = test_db();
        assert_eq!(chain.get_height().unwrap(), 0);

        let genesis: Block<TestTransaction> = chain.get_block(0).unwrap();
        let genesis_last: Block<TestTransaction> = chain.get_last_block().unwrap();
        assert_eq!(genesis.header.bits, blockchain_control::DEFAULT_DIFFICULTY);
        assert_eq!(genesis.header.bits, genesis_last.header.bits)
    }

    #[test]
    fn test_blockchain_persistence() {
        log_init();

        let mut chain = test_db();
        (0..3).into_iter().for_each(|_| {test_add(&mut chain);thread::sleep(Duration::from_secs(1));});

        assert_eq!(chain.get_height().unwrap(), 3);
        let last:Block<TestTransaction> = chain.get_last_block().unwrap();
        eprintln!("{}",last);
    }
}
