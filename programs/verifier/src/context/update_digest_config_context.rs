use crate::errors::ErrorCode;
use crate::state::{DigestConfig, VerifierAccount};
use anchor_lang::prelude::*;

/// Used by `update_config`, `activate_config` and `deactivate_config`
#[derive(Accounts)]
#[instruction(config_digest: [u8; 32])]
pub struct UpdateDigestConfigContext<'info> {
    #[account(seeds = [b"verifier"], bump)]
    pub verifier_account: AccountLoader<'info, VerifierAccount>,
    #[account(mut, seeds = [config_digest.as_ref()], bump = config_account.bump)]
    pub config_account: Account<'info, DigestConfig>,
    #[account(address = verifier_account.load()?.verifier_account_config.owner @ ErrorCode::Unauthorized)]
    pub owner: Signer<'info>,
}
