use crate::common::fixtures::{fixture_signers, FIXTURE_DIGEST, FIXTURE_F, FIXTURE_REPORT};
use crate::common::test_setup::{VerifierTestSetup, VerifierTestSetupBuilder};
use anchor_lang::error::ErrorCode::AccountNotInitialized;
use solana_program_test::tokio;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use test_utils::assert::Assert;
use test_utils::report::test_config_digest;
use verifier::errors::ErrorCode;
use verifier::events::{ConfigActivated, ConfigDeactivated};
use verifier::util::{Compressor, LogParser};

pub mod common;

#[tokio::test]
async fn test_deactivate_and_activate_config() {
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
        .set_config(
            &mut environment_context,
            &user,
            FIXTURE_DIGEST,
            fixture_signers(),
            FIXTURE_F,
        )
        .await;
    Assert::transaction_ok(&result);

    let compressed_report = Compressor::compress(&FIXTURE_REPORT);

    // Deactivate
    let result = verifier_client
        .deactivate_config(&mut environment_context, &user, FIXTURE_DIGEST)
        .await;
    Assert::transaction_ok(&result);

    let event: Option<ConfigDeactivated> = LogParser::parse_logs(result.unwrap().metadata.unwrap().log_messages);
    assert_eq!(event.expect("ConfigDeactivated event should be emitted").config_digest, FIXTURE_DIGEST);

    let config = verifier_client
        .read_digest_config(&mut environment_context, FIXTURE_DIGEST)
        .await
        .unwrap();
    assert!(!config.is_active);

    // Report verification should fail with inactive config
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report.clone(), None)
        .await;
    Assert::transaction_error(&result, ErrorCode::ConfigDeactivated);

    // Activate
    let result = verifier_client
        .activate_config(&mut environment_context, &user, FIXTURE_DIGEST)
        .await;
    Assert::transaction_ok(&result);

    let event: Option<ConfigActivated> = LogParser::parse_logs(result.unwrap().metadata.unwrap().log_messages);
    assert_eq!(event.expect("ConfigActivated event should be emitted").config_digest, FIXTURE_DIGEST);

    let config = verifier_client
        .read_digest_config(&mut environment_context, FIXTURE_DIGEST)
        .await
        .unwrap();
    assert!(config.is_active);

    // Report verification should pass with active config
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;
    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_deactivate_config_only_affects_its_digest() {
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

    // Same signers registered for two digests
    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, FIXTURE_DIGEST, fixture_signers(), FIXTURE_F)
            .await,
    );
    let other_digest = test_config_digest(1);
    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, other_digest, fixture_signers(), FIXTURE_F)
            .await,
    );

    Assert::transaction_ok(
        &verifier_client
            .deactivate_config(&mut environment_context, &user, other_digest)
            .await,
    );

    // The fixture report is for FIXTURE_DIGEST, which is still active
    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&FIXTURE_REPORT), None)
        .await;
    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_activate_and_deactivate_unknown_digest() {
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
        .activate_config(&mut environment_context, &user, test_config_digest(1))
        .await;
    Assert::transaction_error(&result, AccountNotInitialized);

    let result = verifier_client
        .deactivate_config(&mut environment_context, &user, test_config_digest(1))
        .await;
    Assert::transaction_error(&result, AccountNotInitialized);
}

#[tokio::test]
async fn test_activate_and_deactivate_with_non_owner() {
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
        .deactivate_config(&mut environment_context, &non_owner, FIXTURE_DIGEST)
        .await;
    Assert::transaction_error(&result, ErrorCode::Unauthorized);

    let result = verifier_client
        .activate_config(&mut environment_context, &non_owner, FIXTURE_DIGEST)
        .await;
    Assert::transaction_error(&result, ErrorCode::Unauthorized);
}
