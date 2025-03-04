#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use crate::{
        block::{Block, ProofWork},
        hash::Hashable, transaction::Transaction,
    };

	const TEST_BITS:u32 = 0x1f00_ffff;

    #[derive(Debug,Serialize,Deserialize)]
    pub struct TestTransaction;

    impl Hashable for TestTransaction {
        fn hash(&self) -> [u8; 32] {
			[0u8;32]
        }
    }

    impl Transaction for TestTransaction {}

    #[test]
    fn test_pow_validation() {
        let mut block: Block<TestTransaction> = Block::genesis();
        block.header.bits = TEST_BITS;
        block.mine();

        let pow = ProofWork::test_bits(block.header.bits);
        assert!(pow.is_valid(&block.header.hash()))
    }
}
