pub mod common;
pub mod context;
pub mod domain;
pub mod errors;
pub mod events;
pub mod evm;
pub mod state;
pub mod util;

use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak::hash as keccak256;
use common::*;
use context::*;
use domain::*;
use events::*;
use evm::*;
use state::*;
use util::*;

use evm::Encoder;
use std::cell::Ref;

declare_id!("Gt9S41PtjR58CbG9JhJ3J6vxesqrNAswbWYbLNTMZA3c");

#[program]
pub mod verifier {
    use super::*;
    use solana_program::program::{set_return_data};

    pub fn verify(ctx: Context<VerifyContext>, signed_report: Vec<u8>) -> Result<()> {
        let verifier_account = ctx.accounts.verifier_account.load()?;

        let report_data = verify_report(&ctx, &signed_report, &verifier_account)?;

        set_return_data(&report_data);

        Ok(())
    }

    /// Sets the DON configuration for a config digest. Creates the config PDA derived from `[config_digest]`
    pub fn set_config(
        ctx: Context<SetConfigContext>,
        config_digest: [u8; 32],
        signers: Vec<[u8; 20]>,
        f: u8,
    ) -> Result<()> {
        let config_account = &mut ctx.accounts.config_account;

        // An initialized config account always has a non-zero version
        require!(
            config_account.version == 0,
            errors::ErrorCode::ConfigDigestAlreadySet
        );

        let sorted_signers = validate_config(&config_digest, signers, f)?;

        config_account.version = DIGEST_CONFIG_VERSION;
        config_account.config_digest = config_digest;
        config_account.f = f;
        config_account.is_active = true;
        config_account.bump = ctx.bumps.config_account;
        config_account.signers = sorted_signers.clone();

        emit!(ConfigSet {
            config_digest,
            signers: sorted_signers,
            f,
        });

        Ok(())
    }

    /// Replaces the signers and fault tolerance of an existing config digest. The active state is unchanged
    pub fn update_config(
        ctx: Context<UpdateDigestConfigContext>,
        config_digest: [u8; 32],
        signers: Vec<[u8; 20]>,
        f: u8,
    ) -> Result<()> {
        let sorted_signers = validate_config(&config_digest, signers, f)?;

        let config_account = &mut ctx.accounts.config_account;
        config_account.f = f;
        config_account.signers = sorted_signers.clone();

        emit!(ConfigUpdated {
            config_digest,
            signers: sorted_signers,
            f,
        });

        Ok(())
    }

    pub fn activate_config(
        ctx: Context<UpdateDigestConfigContext>,
        config_digest: [u8; 32],
    ) -> Result<()> {
        ctx.accounts.config_account.is_active = true;
        emit!(ConfigActivated { config_digest });
        Ok(())
    }

    pub fn deactivate_config(
        ctx: Context<UpdateDigestConfigContext>,
        config_digest: [u8; 32],
    ) -> Result<()> {
        ctx.accounts.config_account.is_active = false;
        emit!(ConfigDeactivated { config_digest });
        Ok(())
    }

    /// Closes the config PDA for a config digest and refunds its rent to the owner
    pub fn close_config(
        _ctx: Context<CloseConfigContext>,
        config_digest: [u8; 32],
    ) -> Result<()> {
        emit!(ConfigClosed { config_digest });
        Ok(())
    }

    /// Enables or disables falling back to the legacy `don_configs` when no config PDA exists for a report's digest
    pub fn set_legacy_fallback(ctx: Context<UpdateConfigContext>, enabled: bool) -> Result<()> {
        let mut verifier_account = ctx.accounts.verifier_account.load_mut()?;
        verifier_account.legacy_fallback_disabled = u8::from(!enabled);
        emit!(LegacyFallbackSet { enabled });
        Ok(())
    }

    /// Used to Set the access controller
    /// We use an optional access controller.
    /// See https://github.com/coral-xyz/anchor/pull/2101 on how option works in Anchor
    pub fn set_access_controller(ctx: Context<SetAccessControllerContext>) -> Result<()> {
        let mut verifier_account = ctx.accounts.verifier_account.load_mut()?;

        verifier_account.verifier_account_config.access_controller = ctx.accounts.access_controller
            .as_ref()
            .map_or(Pubkey::default(), |ac| ac.key());

        emit!(AccessControllerSet {
            access_controller: verifier_account.verifier_account_config.access_controller.key()
        });
        Ok(())
    }

    /// initialize into existence the verifier account. You must realloc after this
    pub fn initialize(
        _ctx: Context<InitializeContext>
    ) -> Result<()> {
        // Will init PDA but does not attempt to load the account struct as PDA size is too small
        Ok(())
    }

    /// Initializes the verifier (admin) account data. Call after initialize + realloc
    /// We use an optional access controller.
    /// See https://github.com/coral-xyz/anchor/pull/2101 on how option works in Anchor
    pub fn initialize_account_data(
        ctx: Context<InitializeAccountDataContext>
    ) -> Result<()> {
        let mut verifier_account = ctx.accounts.verifier_account.load_mut()?;
        require!(verifier_account.version == 0, errors::ErrorCode::InvalidInputs); // assert uninitialized state
        verifier_account.version = 1;
        if let Some(access_controller) = &ctx.accounts.access_controller {
            verifier_account.verifier_account_config.access_controller = access_controller.key();
        }
        verifier_account.verifier_account_config.owner = ctx.accounts.owner.key();
        Ok(())
    }

    pub fn realloc_account(_ctx: Context<ReallocContext>, _len: u32) -> Result<()> {
        msg!("Reallocated to len: {}", _len as usize);
        Ok(())
    }

    pub fn transfer_ownership(
        ctx: Context<TransferOwnershipContext>,
        proposed_owner: Pubkey,
    ) -> Result<()> {
        let mut verifier_account = ctx.accounts.verifier_account.load_mut()?;
        verifier_account.verifier_account_config.proposed_owner = proposed_owner;
        Ok(())
    }

    pub fn accept_ownership(ctx: Context<AcceptOwnershipContext>) -> Result<()> {
        let mut verifier_account = ctx.accounts.verifier_account.load_mut()?;
        verifier_account.verifier_account_config.owner = std::mem::take(&mut verifier_account.verifier_account_config.proposed_owner);
        Ok(())
    }
}

/// Validates a DON configuration and returns the signers sorted
fn validate_config(config_digest: &[u8; 32], signers: Vec<[u8; 20]>, f: u8) -> Result<Vec<[u8; 20]>> {
    require!(
        *config_digest != [0u8; 32],
        errors::ErrorCode::DigestEmpty
    );

    require!(f > 0, errors::ErrorCode::FaultToleranceMustBePositive);

    require!(
        signers.len() > 3 * f as usize,
        errors::ErrorCode::InsufficientSigners
    );

    require!(
        signers.len() <= MAX_NUMBER_OF_ORACLES as usize,
        errors::ErrorCode::ExcessSigners
    );

    let mut sorted_signers = signers;
    sorted_signers.sort_unstable();

    // Check for duplicate addresses in signers.
    require!(
        !SliceUtil::has_duplicates_sorted(&sorted_signers),
        errors::ErrorCode::NonUniqueSignatures
    );

    for signer in sorted_signers.iter() {
        require!(
            !is_zero_address(signer),
            errors::ErrorCode::ZeroAddress
        );
    }

    Ok(sorted_signers)
}

fn verify_report(
    ctx: &Context<VerifyContext>,
    signed_report: &[u8],
    verifier_account: &Ref<VerifierAccount>
) -> Result<Vec<u8>> {

    let decompressed_report = Compressor::decompress(signed_report);

    let SignedReport {
        report_context,
        report_data,
        rs,
        ss,
        raw_vs,
    } = Encoder::parse_signed_report(&decompressed_report)?;

    let (expected_config_account, _) = Pubkey::find_program_address(&[&report_context[0]], &ID);
    require!(
        expected_config_account == ctx.accounts.config_account.key(),
        errors::ErrorCode::InvalidConfigAccount
    );

    // Validate signature lengths
    require!(
        rs.len() == ss.len(),
        errors::ErrorCode::MismatchedSignatures
    );

    // Ensure there is at least one signer
    require!(
        !rs.is_empty(),
        errors::ErrorCode::NoSigners
    );

    let signed_payload_hash = keccak256(&[
        &keccak256(report_data).to_bytes()[..],
        &report_context[0][..],
        &report_context[1][..],
        &report_context[2][..]
    ].concat());

    // Recover signer addresses from signatures
    let mut signers = Vec::with_capacity(rs.len());
    for i in 0..rs.len() {
        let addr = ecrecover(
            &signed_payload_hash.to_bytes(),
            &rs[i],
            &ss[i],
            raw_vs[i],
        ).map_err(|_| errors::ErrorCode::BadVerification)?;
        require!(
            !is_zero_address(&addr),
            errors::ErrorCode::BadVerification
        );
        signers.push(addr);
    }

    // Checking for duplicate signatures
    require!(
        !SliceUtil::has_duplicates(&signers),
        errors::ErrorCode::BadVerification
    );

    // Parse report details from report_data
    let report = Encoder::parse_report_details_from_report(report_data)?;

    let (f, registered_signers) = if ctx.accounts.config_account.owner == &ID {
        // The config PDA for the report's digest exists
        let data = ctx.accounts.config_account.try_borrow_data()?;
        let digest_config = DigestConfig::try_deserialize(&mut &data[..])?;

        require!(
            digest_config.config_digest == report_context[0],
            errors::ErrorCode::InvalidConfigAccount
        );

        require!(
            digest_config.is_active,
            errors::ErrorCode::ConfigDeactivated
        );

        (digest_config.f, digest_config.signers)
    } else if verifier_account.legacy_fallback_disabled == 0 {
        // No config PDA for the report's digest. Find the legacy DON configuration based on the report timestamp
        msg!("legacy config fallback");

        let active_don_config = verifier_account
            .don_configs
            .iter()
            .rev()
            .find(|config| config.activation_time <= report.report_timestamp)
            .ok_or(errors::ErrorCode::BadVerification)?;

        // Ensure the active DON is indeed active
        require!(
            active_don_config.is_active != 0,
            errors::ErrorCode::ConfigDeactivated
        );

        (
            active_don_config.f,
            active_don_config.signers.iter().map(|s| s.key).collect::<Vec<[u8; 20]>>(),
        )
    } else {
        return Err(errors::ErrorCode::DigestNotSet.into());
    };

    // Verify that the number of signers exceeds the threshold 'f'
    require!(
        signers.len() > f as usize,
        errors::ErrorCode::BadVerification
    );

    // Check each signer is registered
    for signer in signers.iter() {
        if !registered_signers.iter().any(|registered| signer == registered) {
            return Err(errors::ErrorCode::BadVerification.into());
        }
    }

    emit!(ReportVerified {
        feed_id: *report.feed_id,
        requester: ctx.accounts.user.key(),
    });

    Ok(report_data.to_vec())
}
