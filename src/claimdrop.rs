use cosmwasm_std::{Timestamp, Uint128};
use serde::{Deserialize, Serialize};

/// Execute messages for the claimdrop contract
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimdropExecuteMsg {
    /// Claim the airdrop tokens for the sender
    Claim {},
    /// Update configuration (admin only)
    UpdateConfig {
        /// New merkle root for the airdrop
        merkle_root: Option<String>,
        /// New claim start time
        start_time: Option<Timestamp>,
        /// New claim end time  
        end_time: Option<Timestamp>,
        /// New admin address
        admin: Option<String>,
    },
    /// Pause/unpause claims (admin only)
    SetClaimsPaused { paused: bool },
}

/// Query messages for the claimdrop contract
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimdropQueryMsg {
    /// Get the contract configuration
    Config {},
    /// Get claimable amount for a specific address
    ClaimableAmount { address: String },
    /// Check if an address has claimed
    IsClaimed { address: String },
    /// Get claim status and details for an address
    ClaimStatus { address: String },
    /// Get total claimed amount
    TotalClaimed {},
    /// Get merkle proof for verification (if applicable)
    MerkleProof { address: String },
}

/// Response for the config query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    /// The admin address
    pub admin: String,
    /// The merkle root of the airdrop
    pub merkle_root: String,
    /// The claim start time
    pub start_time: Timestamp,
    /// The claim end time
    pub end_time: Timestamp,
    /// Whether claims are paused
    pub claims_paused: bool,
    /// The native token denom for claims
    pub claim_denom: String,
}

/// Response for claimable amount query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimableAmountResponse {
    /// The amount that can be claimed
    pub amount: Uint128,
    /// The denom of the claimable token
    pub denom: String,
    /// Whether the address is eligible for claiming
    pub is_eligible: bool,
    /// The merkle proof for verification
    pub proof: Option<Vec<String>>,
}

/// Response for claim status query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimStatusResponse {
    /// Whether the address has claimed
    pub is_claimed: bool,
    /// The amount claimed (0 if not claimed)
    pub claimed_amount: Uint128,
    /// The claimable amount
    pub claimable_amount: Uint128,
    /// The denom of the token
    pub denom: String,
    /// Whether the address is eligible
    pub is_eligible: bool,
    /// The time when the claim was made (if claimed)
    pub claim_time: Option<Timestamp>,
}

/// Response for total claimed query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TotalClaimedResponse {
    /// Total amount claimed across all users
    pub total_claimed: Uint128,
    /// The denom of the claimed token
    pub denom: String,
    /// Number of unique claimers
    pub total_claimers: u64,
}

/// Response for merkle proof query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProofResponse {
    /// The merkle proof for the address
    pub proof: Vec<String>,
    /// The amount for this address in the merkle tree
    pub amount: Uint128,
    /// Whether the proof is valid
    pub is_valid: bool,
}

/// Simple response for boolean queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsClaimedResponse {
    /// Whether the address has claimed
    pub is_claimed: bool,
}