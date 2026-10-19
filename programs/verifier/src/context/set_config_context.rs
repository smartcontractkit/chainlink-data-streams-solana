use crate::errors::ErrorCode;
use crate::state::{DigestConfig, VerifierAccount};
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(config_digest: [u8; 32])]
pub struct SetConfigContext<'info> {
    #[account(seeds = [b"verifier"], bump)]
    pub verifier_account: AccountLoader<'info, VerifierAccount>,
    // `init_if_needed` so that an existing digest fails with `ConfigDigestAlreadySet` in the handler
    #[account(
        init_if_needed,
        payer = owner,
        space = 8 + DigestConfig::INIT_SPACE,
        seeds = [config_digest.as_ref()],
        bump
    )]
    pub config_account: Account<'info, DigestConfig>,
    #[account(mut, address = verifier_account.load()?.verifier_account_config.owner @ ErrorCode::Unauthorized)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}
