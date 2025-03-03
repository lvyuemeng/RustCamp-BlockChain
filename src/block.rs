use crate::transaction::Transaction;

#[derive(Debug,Clone)]
pub struct Block<T:Transaction> {
	pub header:BlockHeader,
	pub txs:Vec<T>	
}

#[derive(Debug,Clone)]
pub struct BlockHeader {
	pub prev_hash:String,
	pub merkle_root:String,
	pub timestamp:u64,
	// Difficulty Goal
	pub bits:u32,
	pub nonce:u64,
}

