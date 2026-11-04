use anchor_lang::prelude::*;

#[event]
pub struct ConfigActivated {
    pub config_digest: [u8; 32],
}

#[event]
pub struct ConfigDeactivated {
    pub config_digest: [u8; 32],
}

#[event]
pub struct ConfigClosed {
    pub config_digest: [u8; 32],
}

#[event]
pub struct ReportVerified {
    pub feed_id: [u8; 32],
    pub requester: Pubkey,
}

#[event]
pub struct ConfigSet {
    pub config_digest: [u8; 32],
    pub signers: Vec<[u8; 20]>,
    pub f: u8,
}

#[event]
pub struct ConfigUpdated {
    pub config_digest: [u8; 32],
    pub signers: Vec<[u8; 20]>,
    pub f: u8,
}

#[event]
pub struct LegacyFallbackSet {
    pub enabled: bool,
}

#[event]
pub struct AccessControllerSet {
    pub access_controller: Pubkey,
}