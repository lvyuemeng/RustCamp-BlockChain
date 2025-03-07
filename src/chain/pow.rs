use anyhow::Result;
use serde::Deserialize;

use crate::block::{Block, Transaction, pow::PoW};
use crate::chain::{BlockChain, DbKeys, blockchain_control, consensus::PoWConfig};
use crate::hash::{bits_to_target, target_to_bits};

impl BlockChain<PoWConfig> {
    pub fn adjust_difficulty<T: Transaction + for<'a> Deserialize<'a>>(&mut self) -> Result<u32> {
        let height = self.get_height()?;
        if height % self.cs.difficulty_adjust_interval != 0 || height == 0 {
            return Ok(self.cs.cur_bits);
        }
        let first_block: Block<T, PoW> =
            self.get_block(height - blockchain_control::DIFFICULTY_ADJUST_INTERVAL)?;
        let last_block: Block<T, PoW> = self.get_last_block()?;

        let actual_span = (last_block.header.timestamp - first_block.header.timestamp).max(1);

        let prev_target = bits_to_target(first_block.header.proof.bits);
        let new_target =
            prev_target.clone() * blockchain_control::TARGET_TIME_SPAN / actual_span as u64;
        let new_target = new_target.clamp(prev_target.clone() / 4u32, prev_target.clone() * 4u32);
        let new_bits = target_to_bits(new_target);

        self.db.put(DbKeys::CUR_STATE, &new_bits.to_le_bytes())?;
        self.cs.cur_bits = new_bits;
        Ok(new_bits)
    }
}
