use crate::errors::ErrorCode;
use crate::state::{DigestConfig, VerifierAccount};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(config_digest: [u8; 32])]
pub struct CloseConfigContext<'info> {
    #[account(seeds = [b"verifier"], bump)]
    pub verifier_account: AccountLoader<'info, VerifierAccount>,
    // Rent is refunded to the owner
    #[account(mut, close = owner, seeds = [config_digest.as_ref()], bump = config_account.bump)]
    pub config_account: Account<'info, DigestConfig>,
    #[account(mut, address = verifier_account.load()?.verifier_account_config.owner @ ErrorCode::Unauthorized)]
    pub owner: Signer<'info>,
}
