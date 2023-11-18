// Only run this as a WASM if the export-abi feature is not set.
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

use alloy_primitives::{Address, FixedBytes, U128, U32, U64, U16, U8};
// use ethers::core::k256::ecdsa;
use parity_scale_codec::{Compact, CompactAs, Decode, Encode, EncodeAppend, Error, HasCompact};
use stylus_sdk::crypto::keccak;
use stylus_sdk::storage::{
    StorageFixedBytes, StorageKey, StorageMap, StorageU128, StorageU16, StorageU256, StorageU32,
    StorageU64, StorageU8, StorageVec,
};
use alloy_sol_types::sol_data::Uint;
use stylus_sdk::abi::AbiType;

/// Initializes a custom, global allocator for Rust programs compiled to WASM.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// Import the Stylus SDK along with alloy primitive types for use in our program.
use stylus_sdk::{alloy_primitives::U256, prelude::*};

type PayloadItemTuple = ([u8; 2], Vec<u8>);
type CommitmentTuple = (u32, u64, Vec<PayloadItemTuple>);

impl From<CommitmentTuple> for Commitment {
    fn from(value: CommitmentTuple) -> Self {
        Commitment {
            block_number: value.0,
            validator_set_id: value.1,
            payload: value.2.into_iter().map(|v| PayloadItem::from(v)).collect(),
        }
    }
}

impl From<PayloadItemTuple> for PayloadItem {
    fn from(value: PayloadItemTuple) -> Self {
        PayloadItem {
            payload_id: value.0,
            data: value.1,
        }
    }
}
/**
 * @dev The Commitment, with its payload, is the core thing we are trying to verify with
 * this contract. It contains an MMR root that commits to the polkadot history, including
 * past blocks and parachain blocks and can be used to verify both polkadot and parachain blocks.
 */

pub struct Commitment {
    // Relay chain block number
    block_number: u32,
    // ID of the validator set that signed the commitment
    validator_set_id: u64,
    // The payload of the new commitment in beefy justifications (in
    // our case, this is a new MMR root for all past polkadot blocks)
    payload: Vec<PayloadItem>,
}

impl Commitment {
    pub fn encode(&self) -> Vec<u8> {
        let mut result = Vec::new();
        Encode::encode_to(&(self.payload.len() as u32), &mut result);
        self.payload.iter().for_each(|p| {
            let mut p = p.encode();
            result.append(&mut p);
        });
        Encode::encode_to(&self.block_number, &mut result);
        Encode::encode_to(&self.validator_set_id, &mut result);
        result
    }
}

/**
 * @dev Each PayloadItem is a piece of data signed by validators at a particular block.
 */
pub struct PayloadItem {
    // An ID that references a description of the data in the payload item.
    // Known payload ids can be found [upstream](https://github.com/paritytech/substrate/blob/fe1f8ba1c4f23931ae89c1ada35efb3d908b50f5/primitives/consensus/beefy/src/payload.rs#L27).
    pub payload_id: [u8; 2],
    // The contents of the payload item
    pub data: Vec<u8>,
}

impl PayloadItem {
    pub fn encode(&self) -> Vec<u8> {
        let mut result = Vec::new();
        let mut payload_id = self.payload_id.to_vec();
        result.append(&mut payload_id);
        Encode::encode_to(&(self.data.len() as u32), &mut result);
        let mut data = self.data.clone();
        result.append(&mut data);
        result
    }
}

/**
 * @dev The ValidatorSetState describes a BEEFY validator set along with signature usage counters
 */
#[solidity_storage]
pub struct ValidatorSetState {
    // Identifier for the set
    pub id: StorageU128,
    // Number of validators in the set
    pub length: StorageU128,
    // Merkle root of BEEFY validator addresses
    pub root: StorageFixedBytes<32>,
    // Number of times a validator signature has been used
    pub usage_counters: StorageVec<StorageU16>,
}

#[solidity_storage]
pub struct ValidatorSet {
    // Identifier for the set
    pub id: StorageU128,
    // Number of validators in the set
    pub length: StorageU128,
    // Merkle root of BEEFY validator addresses
    pub root: StorageFixedBytes<32>,
}
#[solidity_storage]

pub struct Ticket {
    // The block number this ticket was issued
    pub block_number: StorageU64,
    // Length of the validator set that signed the commitment
    pub validator_set_len: StorageU32,
    // The number of signatures required
    pub num_required_signatures: StorageU32,
    // The PREVRANDAO seed selected for this ticket session
    pub prev_randao: StorageU256,
    // Hash of a bitfield claiming which validators have signed
    pub bitfield_hash: StorageFixedBytes<32>,
}

struct ValidatorProof {
    // The parity bit to specify the intended solution
    v: u8,
    // The x component on the secp256k1 curve
    r: [u8; 32],
    // The challenge solution
    s: [u8; 32],
    // Leaf index of the validator address in the merkle tree
    index: U256,
    // Validator address
    account: Address,
    // Merkle proof for the validator
    proof: Vec<[u8; 32]>,
}
type ValidatorProofTuple = (u8, [u8; 32], [u8; 32], U256, Address, Vec<[u8; 32]>);
impl From<ValidatorProofTuple> for ValidatorProof {
    fn from(value: ValidatorProofTuple) -> Self {
        ValidatorProof {
            v: value.0,
            r: value.1,
            s: value.2,
            index: value.3,
            account: value.4,
            proof: value.5,
        }
    }
}
#[solidity_storage]
pub struct LightClient {
    // /// @dev The latest verified MMR root
    // bytes32 public latestMMRRoot;
    pub latest_mmr_root: StorageFixedBytes<32>,

    // /// @dev The block number in the relay chain in which the latest MMR root was emitted
    // uint64 public latestBeefyBlock;
    pub latest_beefy_block: StorageU64,

    // /// @dev State of the current validator set
    pub current_validator_set: ValidatorSetState,
    // /// @dev State of the next validator set
    pub next_validator_set: ValidatorSetState,

    // /// @dev Pending tickets for commitment submission
    // mapping(bytes32 ticketID => Ticket) public tickets;
    pub tickets: StorageMap<FixedBytes<32>, Ticket>,
}
#[external]
impl LightClient {
    pub fn submit_initial(
        &mut self,
        commitment: CommitmentTuple,
        bitfield: Vec<U256>,
        proof: ValidatorProofTuple,
    ) -> Result<(), Vec<u8>> {
        let commitment = Commitment::from(commitment);
        // let mut vset = ValidatorSetState::default();
        let (signature_usage_count, vset) =
            if U128::from(commitment.validator_set_id) == *self.current_validator_set.id {
                let signature_usage_count = self
                    .current_validator_set
                    .usage_counters
                    .get(1 as usize)
                    .ok_or("ERROR".as_bytes().to_vec())?;
                let mut setter = self
                    .current_validator_set
                    .usage_counters
                    // TODO: Saturating add
                    .setter(1 as usize)
                    .ok_or("ERROR".as_bytes().to_vec())?;
                setter.set(signature_usage_count + U16::from(1));
                // vset = currentValidatorSet;
                (signature_usage_count, &self.current_validator_set)
            } else if U128::from(commitment.validator_set_id) == *self.next_validator_set.id {
                let signature_usage_count = self
                    .next_validator_set
                    .usage_counters
                    .get(1 as usize)
                    .ok_or("ERROR".as_bytes().to_vec())?;
                let mut setter = self
                    .next_validator_set
                    .usage_counters
                    // TODO: Saturating add
                    .setter(1 as usize)
                    .ok_or("ERROR".as_bytes().to_vec())?;
                setter.set(signature_usage_count + U16::from(1));
                // vset = nextValidatorSet;
                (signature_usage_count, &self.next_validator_set)
            } else {
                return Err("InvalidCommitment".as_bytes().to_vec());
            };
        // Check if merkle proof is valid based on the validatorSetRoot and if proof is included in bitfield
        //  if (!isValidatorInSet(vset, proof.account, proof.index, proof.proof) || !Bitfield.isSet(bitfield, proof.index))

        if U128::from(commitment.validator_set_id) != *vset.id {
            return Err("InvalidCommitment".as_bytes().to_vec());
        }

        // Check if validatorSignature is correct, ie. check if it matches
        // the signature of senderPublicKey on the commitmentHash
        let commitement_hash = keccak(commitment.encode());
        let proof = ValidatorProof::from(proof);
        // let signature = ecdsa::Signature::from_scalars(proof.r, proof.s).map_err(|e| {
        //     Err("InvalidSignature".as_bytes().to_vec());
        // })?;
        // secp256k1::recover(&commitement_hash, &proof[0], &proof[1], &proof[2]);
        // let commitement_hash = keccak(commitment)
        //         bytes32 commitmentHash = keccak256(encodeCommitment(commitment));
        //         if (ECDSA.recover(commitmentHash, proof.v, proof.r, proof.s) != proof.account) {
        //             revert InvalidSignature();
        //         }

        //         // For the initial submission, the supplied bitfield should claim that more than
        //         // two thirds of the validator set have sign the commitment
        //         if (Bitfield.countSetBits(bitfield) < computeQuorum(vset.length)) {
        //             revert NotEnoughClaims();
        //         }

        //         tickets[createTicketID(msg.sender, commitmentHash)] = Ticket({
        //             blockNumber: uint64(block.number),
        //             validatorSetLen: uint32(vset.length),
        //             numRequiredSignatures: uint32(
        //                 computeNumRequiredSignatures(vset.length, signatureUsageCount, minNumRequiredSignatures)
        //                 ),
        //             prevRandao: 0,
        //             bitfieldHash: keccak256(abi.encodePacked(bitfield))
        //         });
        Ok(())
    }

    pub fn validate_ticket(&self, ticket_id: FixedBytes<32>, commitment: CommitmentTuple, bitfield: Vec<U256>) -> Result<(), Vec<u8>> {

        let ticket = self.tickets.get(ticket_id);
        let commitment: Commitment = commitment.into();

        if *ticket.block_number == U64::from(0) {
            // submitInitial hasn't been called yet
            return Err(b"InvalidTicket".to_vec())
        }

        if *ticket.prev_randao == U256::from(0) {
            // commitPrevRandao hasn't been called yet
            return Err(b"PrevRandaoNotCaptured".to_vec())
        }

        if U64::from(commitment.block_number) <= self.latest_beefy_block.get() {
            // ticket is obsolete
            return Err(b"StaleCommitment".to_vec())
        }
        // TODO: Actually hash the bitfields not a random zero byte array
        if *ticket.bitfield_hash != keccak([0]) {
            // The provided claims bitfield isn't the same one that was
            // passed to submitInitial
            return Err(b"InvalidBitfield".to_vec())
        }

        Ok(())
    }
}

// function submitInitial(Commitment calldata commitment, uint256[] calldata bitfield, ValidatorProof calldata proof)
//         external
//     {
//         ValidatorSetState storage vset;
//         uint16 signatureUsageCount;
//         if (commitment.validatorSetID == currentValidatorSet.id) {
//             signatureUsageCount = currentValidatorSet.usageCounters.get(proof.index);
//             currentValidatorSet.usageCounters.set(proof.index, signatureUsageCount.saturatingAdd(1));
//             vset = currentValidatorSet;
//         } else if (commitment.validatorSetID == nextValidatorSet.id) {
//             signatureUsageCount = nextValidatorSet.usageCounters.get(proof.index);
//             nextValidatorSet.usageCounters.set(proof.index, signatureUsageCount.saturatingAdd(1));
//             vset = nextValidatorSet;
//         } else {
//             revert InvalidCommitment();
//         }

//         // Check if merkle proof is valid based on the validatorSetRoot and if proof is included in bitfield
//         if (!isValidatorInSet(vset, proof.account, proof.index, proof.proof) || !Bitfield.isSet(bitfield, proof.index))
//         {
//             revert InvalidValidatorProof();
//         }

//         if (commitment.validatorSetID != vset.id) {
//             revert InvalidCommitment();
//         }

//         // Check if validatorSignature is correct, ie. check if it matches
//         // the signature of senderPublicKey on the commitmentHash
//         bytes32 commitmentHash = keccak256(encodeCommitment(commitment));
//         if (ECDSA.recover(commitmentHash, proof.v, proof.r, proof.s) != proof.account) {
//             revert InvalidSignature();
//         }

//         // For the initial submission, the supplied bitfield should claim that more than
//         // two thirds of the validator set have sign the commitment
//         if (Bitfield.countSetBits(bitfield) < computeQuorum(vset.length)) {
//             revert NotEnoughClaims();
//         }

//         tickets[createTicketID(msg.sender, commitmentHash)] = Ticket({
//             blockNumber: uint64(block.number),
//             validatorSetLen: uint32(vset.length),
//             numRequiredSignatures: uint32(
//                 computeNumRequiredSignatures(vset.length, signatureUsageCount, minNumRequiredSignatures)
//                 ),
//             prevRandao: 0,
//             bitfieldHash: keccak256(abi.encodePacked(bitfield))
//         });
//     }

// Define the entrypoint as a Solidity storage object, in this case a struct
// called `Counter` with a single uint256 value called `number`. The sol_storage! macro
// will generate Rust-equivalent structs with all fields mapped to Solidity-equivalent
// storage slots and types.
sol_storage! {
    #[entrypoint]
    pub struct Counter {
        uint256 number;
    }
}

/// Define an implementation of the generated Counter struct, defining a set_number
/// and increment method using the features of the Stylus SDK.
#[external]
impl Counter {
    /// Gets the number from storage.
    pub fn number(&self) -> Result<U256, Vec<u8>> {
        Ok(self.number.get())
    }

    /// Sets a number in storage to a user-specified value.
    pub fn set_number(&mut self, new_number: U256) -> Result<(), Vec<u8>> {
        self.number.set(new_number);
        Ok(())
    }

    /// Increments number and updates it values in storage.
    pub fn increment(&mut self) -> Result<(), Vec<u8>> {
        let number = self.number.get();
        self.set_number(number + U256::from(1))
    }
}
