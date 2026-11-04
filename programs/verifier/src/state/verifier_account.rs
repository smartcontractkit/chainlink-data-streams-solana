use std::borrow::Borrow;
use crate::common::MAX_NUMBER_OF_ORACLES;
use anchor_lang::prelude::*;
use arrayvec::arrayvec;

pub const MAX_NUMBER_OF_DON_CONFIGS: usize = 256; // NOTE: Must be ^2
#[zero_copy]
pub struct VerifierAccountConfig {
    // The contract admin account
    pub owner: Pubkey,
    // The proposed owner to be transferred to
    pub proposed_owner: Pubkey,
    // The access controller which restricts who can verify
    pub access_controller: Pubkey,
}

#[zero_copy]
#[derive(Default, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct SigningKey {
    pub key: [u8; 20],
}

impl Borrow<[u8; 20]> for &SigningKey {
    fn borrow(&self) -> &[u8; 20] {
        &self.key
    }
}

#[zero_copy]
#[derive(Default)]
pub struct SigningKeys {
    pub xs: [SigningKey; MAX_NUMBER_OF_ORACLES as usize],
    pub len: u8,
}
arrayvec!(SigningKeys, SigningKey, u8);

#[zero_copy]
pub struct DonConfig {
    // The time the config was set
    pub activation_time: u32,
    pub don_config_id: [u8; 24],
    // Fault tolerance of the DON
    pub f: u8,
    // Whether the config is active
    pub is_active: u8,
    // Pad to Solana word size
    pub _padding: u8,
    // The list of possible signers within this config
    pub signers: SigningKeys,
}

#[zero_copy]
pub struct DonConfigs {
    pub len: u16,
    pub padding: [u8; 6],
    pub xs: [DonConfig; MAX_NUMBER_OF_DON_CONFIGS],

}
arrayvec!(DonConfigs, DonConfig, u16);

#[account(zero_copy)]
pub struct VerifierAccount {
    // Versioning for migrating state
    pub version: u8,
    // 0 = verify falls back to `don_configs` when no config PDA exists for the report's digest.
    // 1 = fallback disabled.
    pub legacy_fallback_disabled: u8,
    // Add padding to ensure 8-byte alignment
    pub padding: [u8; 6],
    // Account configuration
    pub verifier_account_config: VerifierAccountConfig,
    // Legacy DON configurations, looked up by report timestamp. Only read by the legacy fallback in verify
    pub don_configs: DonConfigs
}

impl VerifierAccount {
    pub const INIT_SPACE: usize = 10 * 1024; // 10KB for simplicity
}
