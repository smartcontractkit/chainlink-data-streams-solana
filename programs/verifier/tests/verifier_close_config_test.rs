use crate::common::fixtures::{fixture_signers, FIXTURE_DIGEST, FIXTURE_F, FIXTURE_REPORT};
use crate::common::test_setup::{VerifierTestSetup, VerifierTestSetupBuilder};
use anchor_lang::error::ErrorCode::AccountNotInitialized;
use solana_program_test::tokio;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use test_utils::assert::Assert;
use test_utils::client::LegacyDonConfig;
use test_utils::environment_context_operations::EnvironmentContextOperations;
use test_utils::report::test_config_digest;
use verifier::errors::ErrorCode;
use verifier::events::ConfigClosed;
use verifier::util::{Compressor, LogParser};

pub mod common;

#[tokio::test]
async fn test_close_config() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .build()
        .await;

    Assert::transaction_ok(
        &verifier_client
            .set_legacy_fallback(&mut environment_context, &user, false)
            .await,
    );

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, FIXTURE_DIGEST, fixture_signers(), FIXTURE_F)
            .await,
    );

    let config_account = verifier_client.config_pda(&FIXTURE_DIGEST);
    let rent = EnvironmentContextOperations::get_account(&mut environment_context, config_account)
        .await
        .unwrap()
        .unwrap()
        .lamports;
    let owner_balance_before = EnvironmentContextOperations::get_account(&mut environment_context, user.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;

    let result = verifier_client
        .close_config(&mut environment_context, &user, FIXTURE_DIGEST)
        .await;
    Assert::transaction_ok(&result);

    let event: Option<ConfigClosed> = LogParser::parse_logs(result.unwrap().metadata.unwrap().log_messages);
    assert_eq!(event.expect("ConfigClosed event should be emitted").config_digest, FIXTURE_DIGEST);

    // Account is gone and rent is refunded to the owner (minus the transaction fee)
    assert!(EnvironmentContextOperations::get_account(&mut environment_context, config_account)
        .await
        .unwrap()
        .is_none());
    let owner_balance_after = EnvironmentContextOperations::get_account(&mut environment_context, user.pubkey())
        .await
        .unwrap()
        .unwrap()
        .lamports;
    assert!(owner_balance_after > owner_balance_before);
    assert!(owner_balance_after <= owner_balance_before + rent);

    // Reports for the digest no longer verify
    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&FIXTURE_REPORT), None)
        .await;
    Assert::transaction_error(&result, ErrorCode::DigestNotSet);
}

#[tokio::test]
async fn test_config_can_be_set_again_after_close() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .build()
        .await;

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, FIXTURE_DIGEST, fixture_signers(), FIXTURE_F)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .close_config(&mut environment_context, &user, FIXTURE_DIGEST)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, FIXTURE_DIGEST, fixture_signers(), FIXTURE_F)
            .await,
    );

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&FIXTURE_REPORT), None)
        .await;
    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_closed_config_falls_back_to_legacy_when_enabled() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .build()
        .await;

    // The same signers exist as a legacy config
    verifier_client
        .set_legacy_don_configs(
            &mut environment_context,
            &[LegacyDonConfig {
                signers: fixture_signers(),
                f: FIXTURE_F,
                activation_time: 1,
                is_active: true,
            }],
        )
        .await;

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, FIXTURE_DIGEST, fixture_signers(), FIXTURE_F)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .close_config(&mut environment_context, &user, FIXTURE_DIGEST)
            .await,
    );

    // While the legacy fallback is enabled a closed digest still verifies through the legacy configs
    let compressed_report = Compressor::compress(&FIXTURE_REPORT);
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report.clone(), None)
        .await;
    Assert::transaction_ok(&result);

    // Once the fallback is disabled it no longer does
    Assert::transaction_ok(
        &verifier_client
            .set_legacy_fallback(&mut environment_context, &user, false)
            .await,
    );
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;
    Assert::transaction_error(&result, ErrorCode::DigestNotSet);
}

#[tokio::test]
async fn test_close_unknown_digest() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .build()
        .await;

    let result = verifier_client
        .close_config(&mut environment_context, &user, test_config_digest(1))
        .await;

    Assert::transaction_error(&result, AccountNotInitialized);
}

#[tokio::test]
async fn test_close_config_with_non_owner() {
    let non_owner = Keypair::new();
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .add_user(non_owner.pubkey())
        .build()
        .await;

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, FIXTURE_DIGEST, fixture_signers(), FIXTURE_F)
            .await,
    );

    let result = verifier_client
        .close_config(&mut environment_context, &non_owner, FIXTURE_DIGEST)
        .await;
    Assert::transaction_error(&result, ErrorCode::Unauthorized);

    assert!(verifier_client
        .read_digest_config(&mut environment_context, FIXTURE_DIGEST)
        .await
        .is_some());
}
