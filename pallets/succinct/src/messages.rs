use ark_std::string::String;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{Deserialize, Serialize};
use scale_info::TypeInfo;
use sp_core::H256;
use sp_std::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub struct LightClientStep {
	// TODO U256?
	pub attested_slot: u64,
	pub finalized_slot: u64,
	pub participation: u16,
	pub finalized_header_root: H256,
	pub execution_state_root: H256,
	pub proof: Groth16Proof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub struct Groth16Proof {
	pub a: Vec<String>,
	pub b: Vec<Vec<String>>,
	pub c: Vec<String>,
}

// =========

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Encode, Decode, TypeInfo)]
pub struct LightClientRotate {
	pub step: LightClientStep,
	pub sync_committee_ssz: u64,
	pub sync_committee_poseidon: H256,
	pub proof: Vec<u8>,
}

#[derive(Clone, Copy, Encode, Decode, Debug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct SuccinctConfig {
	pub updater: H256,
	pub genesis_validators_root: H256,
	pub genesis_time: u64,
	pub seconds_per_slot: u64,
	pub slots_per_period: u64,
	pub source_chain_id: u32,
	pub finality_threshold: u16,
	pub head: u64,
	pub consistent: bool,
}

impl Default for SuccinctConfig {
	fn default() -> Self {
		Self {
			updater: H256([0u8; 32]),
			genesis_validators_root: H256([0u8; 32]),
			genesis_time: Default::default(),
			seconds_per_slot: Default::default(),
			slots_per_period: Default::default(),
			source_chain_id: Default::default(),
			finality_threshold: Default::default(),
			head: Default::default(),
			consistent: Default::default(),
		}
	}
}
