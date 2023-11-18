// Only run this as a WASM if the export-abi feature is not set.
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

use alloy_primitives::FixedBytes;
use stylus_sdk::storage::{
    StorageFixedBytes, StorageKey, StorageMap, StorageU128, StorageU16, StorageU256, StorageU32,
    StorageU64, StorageU8, StorageVec,
};

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
            blockNumber: value.0,
            validatorSetID: value.1,
            payload: value.2.into_iter().map(|v| PayloadItem::from(v)).collect(),
        }
    }
}

impl From<PayloadItemTuple> for PayloadItem {
    fn from(value: PayloadItemTuple) -> Self {
        PayloadItem {
            payloadID: value.0,
            data: value.1,
        }
    }
}
/**
 * @dev The Commitment, with its payload, is the core thing we are trying to verify with
 * this contract. It contains an MMR root that commits to the polkadot history, including
 * past blocks and parachain blocks and can be used to verify both polkadot and parachain blocks.
 */

struct Commitment {
    // Relay chain block number
    blockNumber: u32,
    // ID of the validator set that signed the commitment
    validatorSetID: u64,
    // The payload of the new commitment in beefy justifications (in
    // our case, this is a new MMR root for all past polkadot blocks)
    payload: Vec<PayloadItem>,
}

/**
 * @dev Each PayloadItem is a piece of data signed by validators at a particular block.
 */
struct PayloadItem {
    // An ID that references a description of the data in the payload item.
    // Known payload ids can be found [upstream](https://github.com/paritytech/substrate/blob/fe1f8ba1c4f23931ae89c1ada35efb3d908b50f5/primitives/consensus/beefy/src/payload.rs#L27).
    payloadID: [u8; 2],
    // The contents of the payload item
    data: Vec<u8>,
}

/**
 * @dev The ValidatorSetState describes a BEEFY validator set along with signature usage counters
 */
#[solidity_storage]

struct ValidatorSetState {
    // Identifier for the set
    id: StorageU128,
    // Number of validators in the set
    length: StorageU128,
    // Merkle root of BEEFY validator addresses
    root: StorageFixedBytes<32>,
    // Number of times a validator signature has been used
    usage_counters: StorageVec<StorageU16>,
}

#[solidity_storage]
struct ValidatorSet {
    // Identifier for the set
    id: StorageU128,
    // Number of validators in the set
    length: StorageU128,
    // Merkle root of BEEFY validator addresses
    root: StorageFixedBytes<32>,
}
#[solidity_storage]

struct Ticket {
    // The block number this ticket was issued
    blockNumber: StorageU64,
    // Length of the validator set that signed the commitment
    validatorSetLen: StorageU32,
    // The number of signatures required
    numRequiredSignatures: StorageU32,
    // The PREVRANDAO seed selected for this ticket session
    prevRandao: StorageU256,
    // Hash of a bitfield claiming which validators have signed
    bitfieldHash: StorageFixedBytes<32>,
}
#[solidity_storage]
pub struct LightClient {
    // /// @dev The latest verified MMR root
    // bytes32 public latestMMRRoot;
    latest_mmr_root: StorageFixedBytes<32>,

    // /// @dev The block number in the relay chain in which the latest MMR root was emitted
    // uint64 public latestBeefyBlock;
    latest_beefy_block: StorageU64,

    // /// @dev State of the current validator set
    current_validator_set: ValidatorSetState,
    // /// @dev State of the next validator set
    next_velidator_set: ValidatorSetState,

    // /// @dev Pending tickets for commitment submission
    // mapping(bytes32 ticketID => Ticket) public tickets;
    tickets: StorageMap<FixedBytes<32>, Ticket>,
}
#[external]
impl LightClient {
    pub fn submit_initial(
        &self,
        commitment: CommitmentTuple,
        bitfield: Vec<U256>,
        proof: Vec<U256>,
    ) -> Result<(), Vec<u8>> {
        let commitment = Commitment::from(commitment);

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
