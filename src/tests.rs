#[cfg(test)]
mod tests {
    use std::{default, env::temp_dir, thread, time::Duration};

    use chrono::Utc;
    use ed25519_dalek::{
        SECRET_KEY_LENGTH, SecretKey, Signature, Signer, SigningKey, Verifier, VerifyingKey,
    };
    use serde::{Deserialize, Serialize};

    use crate::{
        block::{
            Block, BlockHeader, Consensus, Transaction, Transactions,
            pos::{PoS, PoSTransaction, TransactionType},
            pow::PoW,
        },
        chain::{BlockChain, blockchain_control},
        hash::{Hashable, bits_to_target},
    };

    const TEST_BITS: u32 = 0x1f00_ffff;

    #[derive(Debug, Serialize, Deserialize, Default)]
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

    fn test_new_block<
        T: Transaction + for<'a> Deserialize<'a>,
        C: Consensus + for<'a> Deserialize<'a>,
    >(
        chain: &mut BlockChain<C>,
        txs: Transactions<T>,
    ) -> Block<T, C> {
        let prev: Block<T, C> = chain.get_last_block().unwrap();
        let block = chain.get_consensus().generate_block(&prev, txs).unwrap();
        block
    }

    fn test_add<C: Consensus + for<'a> Deserialize<'a>>(chain: &mut BlockChain<C>) {
        let block = test_new_block(chain, Transactions(vec![TestTransaction]));
        chain.add_block(block).unwrap();
    }

    fn test_db<T: Transaction + Default, C: Consensus + for<'a> Deserialize<'a>>() -> BlockChain<C>
    {
        let random_suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let db_dir = temp_dir().join(format!("blockchain_test_{}", random_suffix));

        std::fs::create_dir_all(&db_dir).unwrap();
        let chain = BlockChain::new::<T>(db_dir).unwrap();

        chain
    }

    #[test]
    fn test_pos() {
        // 使用 PoS 的区块链

        let secret_key_bytes_1: [u8; SECRET_KEY_LENGTH] = [
            157, 097, 177, 157, 239, 253, 090, 096, 186, 132, 074, 244, 146, 236, 044, 196, 068,
            073, 197, 105, 123, 050, 105, 025, 112, 059, 172, 003, 028, 174, 127, 096,
        ];

        let secret_key_bytes_2: [u8; SECRET_KEY_LENGTH] = [
            158, 097, 177, 157, 239, 253, 090, 096, 186, 132, 074, 244, 146, 236, 044, 196, 068,
            073, 197, 105, 123, 050, 105, 025, 112, 059, 172, 003, 028, 174, 127, 096,
        ];

        let secret_key_bytes_3: [u8; SECRET_KEY_LENGTH] = [
            159, 097, 177, 157, 239, 253, 090, 096, 186, 132, 074, 244, 146, 236, 044, 196, 068,
            073, 197, 105, 123, 050, 105, 025, 112, 059, 172, 003, 028, 174, 127, 096,
        ];

        let signing_key: SigningKey = SigningKey::from_bytes(&secret_key_bytes_1);
        assert_eq!(signing_key.to_bytes(), secret_key_bytes_1);

        let mut pos_consensus = PoS::default();
        println!(
            "Added validators: {:?}: {}",
            signing_key.verifying_key(),
            60
        );
        pos_consensus.add_validator(signing_key.to_bytes(), 60);
        let signing_key: SigningKey = SigningKey::from_bytes(&secret_key_bytes_2);
        println!(
            "Added validators: {:?}: {}",
            signing_key.verifying_key(),
            100
        );
        pos_consensus.add_validator(signing_key.to_bytes(), 100);
        let signing_key: SigningKey = SigningKey::from_bytes(&secret_key_bytes_3);
        println!(
            "Added validators: {:?}: {}",
            signing_key.verifying_key(),
            80
        );
        pos_consensus.add_validator(signing_key.to_bytes(), 80);

        let mut pos_chain = test_db::<PoSTransaction, PoS>();
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

    #[test]
    fn test_pow_validation() {
        let chain = test_db::<TestTransaction, PoW>();
        let mut block: Block<TestTransaction, PoW> = chain.get_last_block().unwrap();
        block.header.data.bits = TEST_BITS;
        block.mine();

        // let pow = PoW::test_bits(block.header.data.bits);
        // assert!(pow.is_valid(&block.header.hash()))
    }

    #[test]
    fn test_bits_target_transform() {
        log_init();

        let bits = blockchain_control::DEFAULT_DIFFICULTY;
        let target = bits_to_target(bits);

        log::info!("target: {}", target);
    }

    #[test]
    fn test_blockchain_creation() {
        let chain = test_db::<TestTransaction, PoW>();
        assert_eq!(chain.get_height().unwrap(), 0);

        let genesis: Block<TestTransaction, PoW> = chain.get_block(0).unwrap();
        let genesis_last: Block<TestTransaction, PoW> = chain.get_last_block().unwrap();
        assert_eq!(
            genesis.header.data.bits,
            blockchain_control::DEFAULT_DIFFICULTY
        );
        assert_eq!(genesis.header.data.bits, genesis_last.header.data.bits);

        let chain = test_db::<TestTransaction, PoS>();
        assert_eq!(chain.get_height().unwrap(), 0);

        let genesis: Block<TestTransaction, PoS> = chain.get_block(0).unwrap();
        let genesis_last: Block<TestTransaction, PoS> = chain.get_last_block().unwrap();
        assert_eq!(
            genesis.header.data.validator_key,
            VerifyingKey::from_bytes(&[0; 32]).unwrap(),
        );
        assert_eq!(
            genesis.header.data.validator_key,
            genesis_last.header.data.validator_key
        );
    }

    #[test]
    fn test_blockchain_persistence() {
        log_init();

        let mut chain = test_db::<TestTransaction, PoW>();
        (0..3).into_iter().for_each(|_| {
            let block = chain
                .get_consensus()
                .generate_block(
                    &chain.get_last_block().unwrap(),
                    Transactions(vec![TestTransaction]),
                )
                .unwrap();

            chain.add_block(block);
            thread::sleep(Duration::from_secs(1));
        });

        assert_eq!(chain.get_height().unwrap(), 3);
        let last: Block<TestTransaction, PoW> = chain.get_last_block().unwrap();
        eprintln!("PoW Block {}", last);

        let mut chain = test_db::<TestTransaction, PoS>();

        let secret_key_bytes_1: [u8; SECRET_KEY_LENGTH] = [
            157, 097, 177, 157, 239, 253, 090, 096, 186, 132, 074, 244, 146, 236, 044, 196, 068,
            073, 197, 105, 123, 050, 105, 025, 112, 059, 172, 003, 028, 174, 127, 096,
        ];
        let secret_key_bytes_2: [u8; SECRET_KEY_LENGTH] = [
            158, 097, 177, 157, 239, 253, 090, 096, 186, 132, 074, 244, 146, 236, 044, 196, 068,
            073, 197, 105, 123, 050, 105, 025, 112, 059, 172, 003, 028, 174, 127, 096,
        ];

        chain
            .get_consensus_mut()
            .add_validator(secret_key_bytes_1, 100);
        chain
            .get_consensus_mut()
            .add_validator(secret_key_bytes_2, 20);

        (0..3).into_iter().for_each(|_| {
            let block = chain
                .get_consensus()
                .generate_block(
                    &chain.get_last_block().unwrap(),
                    Transactions(vec![TestTransaction]),
                )
                .unwrap();

            chain.add_block(block);
            thread::sleep(Duration::from_secs(1));
        });

        assert_eq!(chain.get_height().unwrap(), 3);
        let last: Block<TestTransaction, PoS> = chain.get_last_block().unwrap();
        eprintln!("PoS Block {}", last);
    }
}
