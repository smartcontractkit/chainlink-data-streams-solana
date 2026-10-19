use crate::common::test_setup::{VerifierTestSetup, VerifierTestSetupBuilder};
use anchor_lang::error::ErrorCode::AccountNotInitialized;
use solana_program_test::tokio;
use solana_sdk::signature::{Keypair, Signer};
use test_utils::assert::Assert;
use test_utils::report::{generate_report_with_signers_and_digest, get_signers, test_config_digest, V3Report};
use verifier::errors::ErrorCode;
use verifier::events::ConfigUpdated;
use verifier::util::{Compressor, LogParser};

pub mod common;

#[tokio::test]
async fn test_update_config() {
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

    let digest = test_config_digest(1);
    let old_signers = get_signers(16);
    let new_signers = get_signers(7);

    Assert::transaction_ok(
        &verifier_client
            .set_config(
                &mut environment_context,
                &user,
                digest,
                old_signers.iter().map(|s| s.signer_address).collect(),
                5,
            )
            .await,
    );

    let (old_report, _) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, Some(old_signers));
    let (new_report, new_signer_addresses) = generate_report_with_signers_and_digest::<V3Report>(digest, 7, 3, None, Some(new_signers));

    // New signers can not verify yet
    Assert::transaction_error(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&new_report), None)
            .await,
        ErrorCode::BadVerification,
    );

    let result = verifier_client
        .update_config(&mut environment_context, &user, digest, new_signer_addresses.clone(), 2)
        .await;
    Assert::transaction_ok(&result);

    let mut sorted_signers = new_signer_addresses.clone();
    sorted_signers.sort_unstable();

    let event: Option<ConfigUpdated> = LogParser::parse_logs(result.unwrap().metadata.unwrap().log_messages);
    let event = event.expect("ConfigUpdated event should be emitted");
    assert_eq!(event.config_digest, digest);
    assert_eq!(event.signers, sorted_signers);
    assert_eq!(event.f, 2);

    let config = verifier_client
        .read_digest_config(&mut environment_context, digest)
        .await
        .unwrap();
    assert_eq!(config.config_digest, digest);
    assert_eq!(config.f, 2);
    assert_eq!(config.signers, sorted_signers);
    assert!(config.is_active);

    // Old signers no longer verify, new signers do
    Assert::transaction_error(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&old_report), None)
            .await,
        ErrorCode::BadVerification,
    );
    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&new_report), None)
            .await,
    );
}

#[tokio::test]
async fn test_update_config_keeps_active_state() {
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

    let digest = test_config_digest(1);
    let signers: Vec<[u8; 20]> = get_signers(4).iter().map(|s| s.signer_address).collect();

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest, signers.clone(), 1)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .deactivate_config(&mut environment_context, &user, digest)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .update_config(&mut environment_context, &user, digest, signers, 1)
            .await,
    );

    let config = verifier_client
        .read_digest_config(&mut environment_context, digest)
        .await
        .unwrap();
    assert!(!config.is_active);
}

#[tokio::test]
async fn test_update_config_fails_when_digest_not_set() {
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

    let signers: Vec<[u8; 20]> = get_signers(4).iter().map(|s| s.signer_address).collect();

    let result = verifier_client
        .update_config(&mut environment_context, &user, test_config_digest(1), signers, 1)
        .await;

    Assert::transaction_error(&result, AccountNotInitialized);
}

#[tokio::test]
async fn test_update_config_validates_config() {
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

    let digest = test_config_digest(1);
    let signers: Vec<[u8; 20]> = get_signers(4).iter().map(|s| s.signer_address).collect();

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest, signers.clone(), 1)
            .await,
    );

    Assert::transaction_error(
        &verifier_client
            .update_config(&mut environment_context, &user, digest, signers.clone(), 0)
            .await,
        ErrorCode::FaultToleranceMustBePositive,
    );
    Assert::transaction_error(
        &verifier_client
            .update_config(&mut environment_context, &user, digest, signers.clone(), 2)
            .await,
        ErrorCode::InsufficientSigners,
    );

    let mut duplicate_signers = signers.clone();
    duplicate_signers.push(signers[0]);
    Assert::transaction_error(
        &verifier_client
            .update_config(&mut environment_context, &user, digest, duplicate_signers, 1)
            .await,
        ErrorCode::NonUniqueSignatures,
    );

    let mut zero_address_signers = signers.clone();
    zero_address_signers.push([0u8; 20]);
    Assert::transaction_error(
        &verifier_client
            .update_config(&mut environment_context, &user, digest, zero_address_signers, 1)
            .await,
        ErrorCode::ZeroAddress,
    );
}

#[tokio::test]
async fn test_update_config_with_non_owner() {
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

    let digest = test_config_digest(1);
    let signers: Vec<[u8; 20]> = get_signers(4).iter().map(|s| s.signer_address).collect();

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest, signers.clone(), 1)
            .await,
    );

    let result = verifier_client
        .update_config(&mut environment_context, &non_owner, digest, signers, 1)
        .await;

    Assert::transaction_error(&result, ErrorCode::Unauthorized);
}
