#[cfg(test)]
mod tests {
    use crate::{
        block::{Block, ProofWork},
        hash::Hashable, transaction::Transaction,
    };
	const TEST_BITS:u32 = 0x1f00_ffff;
    pub struct DumyTransaction;

    impl Hashable for DumyTransaction {
        fn hash(&self) -> [u8; 32] {
			[0u8;32]
        }
    }

    impl Transaction for DumyTransaction {}

    #[test]
    fn test_pow_validation() {
        let mut block: Block<DumyTransaction> = Block::genesis();
        block.header.bits = TEST_BITS;
        block.mine();

        let pow = ProofWork::test_bits(block.header.bits);
        assert!(pow.is_valid(&block.header.hash()))
    }
}
