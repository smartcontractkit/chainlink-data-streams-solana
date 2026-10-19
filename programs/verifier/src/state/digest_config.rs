use anchor_lang::prelude::*;

pub const DIGEST_CONFIG_VERSION: u8 = 1;

/// DON configuration for a single config digest.
/// Stored in a PDA derived from `[config_digest]`, which is the same account the client passes as
/// `config_account` when verifying a report (seed = first 32 bytes of the uncompressed report).
#[account]
#[derive(InitSpace)]
pub struct DigestConfig {
    // Versioning for migrating state. 0 = uninitialized
    pub version: u8,
    // The OCR config digest this config is keyed by
    pub config_digest: [u8; 32],
    // Fault tolerance of the DON
    pub f: u8,
    // Whether the config is active
    pub is_active: bool,
    // PDA bump
    pub bump: u8,
    // The sorted list of signers within this config
    #[max_len(31)] // MAX_NUMBER_OF_ORACLES
    pub signers: Vec<[u8; 20]>,
    // Reserved for future use
    pub reserved: [u8; 32],
}
