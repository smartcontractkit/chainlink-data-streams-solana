use crate::common::test_setup::{VerifierTestSetup, VerifierTestSetupBuilder};
use solana_program_test::tokio;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use test_utils::assert::Assert;
use test_utils::client::LegacyDonConfig;
use test_utils::report::{
    generate_report_with_signers, generate_report_with_signers_and_digest, get_signers,
    test_config_digest, V3Report,
};
use verifier::errors::ErrorCode;
use verifier::events::LegacyFallbackSet;
use verifier::state::MAX_NUMBER_OF_DON_CONFIGS;
use verifier::util::{Compressor, LogParser};

pub mod common;

fn legacy_config(signers: Vec<[u8; 20]>, f: u8, activation_time: u32) -> LegacyDonConfig {
    LegacyDonConfig {
        signers,
        f,
        activation_time,
        is_active: true,
    }
}

#[tokio::test]
async fn test_legacy_fallback_enabled_by_default() {
    let VerifierTestSetup {
        mut environment_context,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .build()
        .await;

    let verifier_account = verifier_client
        .read_verifier_account(&mut environment_context)
        .await
        .unwrap();

    assert_eq!(verifier_account.legacy_fallback_disabled, 0);
}

#[tokio::test]
async fn test_legacy_fallback_verifies_unregistered_digest() {
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

    let (report, signers) = generate_report_with_signers_and_digest::<V3Report>(test_config_digest(1), 16, 6, None, None);

    verifier_client
        .set_legacy_don_configs(&mut environment_context, &[legacy_config(signers, 5, 1_600_000_000)])
        .await;

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report), None)
        .await;
    Assert::transaction_ok(&result);

    let logs = result.unwrap().metadata.unwrap().log_messages;
    assert!(
        logs.iter().any(|l| l.contains("legacy config fallback")),
        "Verify should log when the legacy fallback is used"
    );
}

#[tokio::test]
async fn test_legacy_fallback_without_configs() {
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

    let (report, _) = generate_report_with_signers_and_digest::<V3Report>(test_config_digest(1), 16, 6, None, None);

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report), None)
        .await;

    Assert::transaction_error(&result, ErrorCode::BadVerification);
}

#[tokio::test]
async fn test_digest_config_takes_precedence_over_legacy() {
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
    let legacy_signers = get_signers(16);
    let digest_signers = get_signers(16);

    verifier_client
        .set_legacy_don_configs(
            &mut environment_context,
            &[legacy_config(legacy_signers.iter().map(|s| s.signer_address).collect(), 5, 1_600_000_000)],
        )
        .await;
    Assert::transaction_ok(
        &verifier_client
            .set_config(
                &mut environment_context,
                &user,
                digest,
                digest_signers.iter().map(|s| s.signer_address).collect(),
                5,
            )
            .await,
    );

    // Signed by the legacy signers, but the digest has its own config
    let (legacy_signed, _) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, Some(legacy_signers));
    Assert::transaction_error(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&legacy_signed), None)
            .await,
        ErrorCode::BadVerification,
    );

    let (digest_signed, _) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, Some(digest_signers));
    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&digest_signed), None)
            .await,
    );
}

#[tokio::test]
async fn test_disable_and_enable_legacy_fallback() {
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

    let signers = get_signers(16);
    let signer_addresses: Vec<[u8; 20]> = signers.iter().map(|s| s.signer_address).collect();

    verifier_client
        .set_legacy_don_configs(&mut environment_context, &[legacy_config(signer_addresses.clone(), 5, 1_600_000_000)])
        .await;

    // A registered digest and an unregistered digest, signed by the same signers
    let registered_digest = test_config_digest(1);
    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, registered_digest, signer_addresses, 5)
            .await,
    );
    let (registered_report, _) = generate_report_with_signers_and_digest::<V3Report>(registered_digest, 16, 6, None, Some(signers.clone()));
    let (unregistered_report, _) = generate_report_with_signers_and_digest::<V3Report>(test_config_digest(2), 16, 6, None, Some(signers));
    let registered_report = Compressor::compress(&registered_report);
    let unregistered_report = Compressor::compress(&unregistered_report);

    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, unregistered_report.clone(), None)
            .await,
    );

    // Disable
    let result = verifier_client
        .set_legacy_fallback(&mut environment_context, &user, false)
        .await;
    Assert::transaction_ok(&result);

    let event: Option<LegacyFallbackSet> = LogParser::parse_logs(result.unwrap().metadata.unwrap().log_messages);
    assert!(!event.expect("LegacyFallbackSet event should be emitted").enabled);

    let verifier_account = verifier_client
        .read_verifier_account(&mut environment_context)
        .await
        .unwrap();
    assert_eq!(verifier_account.legacy_fallback_disabled, 1);

    Assert::transaction_error(
        &verifier_client
            .verify(&mut environment_context, &user, unregistered_report.clone(), None)
            .await,
        ErrorCode::DigestNotSet,
    );
    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, registered_report.clone(), None)
            .await,
    );

    // Enable
    let result = verifier_client
        .set_legacy_fallback(&mut environment_context, &user, true)
        .await;
    Assert::transaction_ok(&result);

    let event: Option<LegacyFallbackSet> = LogParser::parse_logs(result.unwrap().metadata.unwrap().log_messages);
    assert!(event.expect("LegacyFallbackSet event should be emitted").enabled);

    let verifier_account = verifier_client
        .read_verifier_account(&mut environment_context)
        .await
        .unwrap();
    assert_eq!(verifier_account.legacy_fallback_disabled, 0);

    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, unregistered_report, None)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, registered_report, None)
            .await,
    );
}

#[tokio::test]
async fn test_set_legacy_fallback_with_non_owner() {
    let non_owner = Keypair::new();
    let VerifierTestSetup {
        mut environment_context,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .add_user(non_owner.pubkey())
        .build()
        .await;

    let result = verifier_client
        .set_legacy_fallback(&mut environment_context, &non_owner, false)
        .await;

    Assert::transaction_error(&result, ErrorCode::Unauthorized);
}

// The tests below cover the legacy, timestamp based config lookup which is unchanged

#[tokio::test]
async fn test_legacy_can_verify_older_reports_with_older_configs() {
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

    // Report for config A with timestamp between config A and B's activation times
    let (report_a, signers_a) = generate_report_with_signers::<V3Report>(16, 6, Some(1_500_000_000), None);
    let (_, signers_b) = generate_report_with_signers::<V3Report>(16, 6, None, None);

    verifier_client
        .set_legacy_don_configs(
            &mut environment_context,
            &[
                legacy_config(signers_a, 5, 1_400_000_000),
                legacy_config(signers_b, 5, 1_600_000_000),
            ],
        )
        .await;

    // Uses config A since the report timestamp is before config B's activation
    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report_a), None)
        .await;
    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_legacy_rolling_out_configuration() {
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

    let signers_a = get_signers(16);

    // Config B adds one more signer
    let mut signers_b = signers_a.clone();
    signers_b.extend(get_signers(1));

    verifier_client
        .set_legacy_don_configs(
            &mut environment_context,
            &[
                legacy_config(signers_a.iter().map(|s| s.signer_address).collect(), 5, 1_600_000_000),
                legacy_config(signers_b.iter().map(|s| s.signer_address).collect(), 5, 1_700_000_000),
            ],
        )
        .await;

    // Report signed by the last 6 signers of config B, which includes the new signer
    let report_signers = signers_b[signers_b.len() - 6..].to_vec();
    let (report, _) = generate_report_with_signers::<V3Report>(16, 6, None, Some(report_signers));

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report), None)
        .await;
    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_legacy_verify_fails_when_report_is_older_than_config() {
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

    let (report, signers) = generate_report_with_signers::<V3Report>(16, 6, Some(1_500_000_000), None);

    verifier_client
        .set_legacy_don_configs(&mut environment_context, &[legacy_config(signers, 5, 1_600_000_000)])
        .await;

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report), None)
        .await;
    Assert::transaction_error(&result, ErrorCode::BadVerification);
}

#[tokio::test]
async fn test_legacy_deactivated_config() {
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

    let (report, signers) = generate_report_with_signers::<V3Report>(16, 6, None, None);

    verifier_client
        .set_legacy_don_configs(
            &mut environment_context,
            &[LegacyDonConfig {
                signers,
                f: 5,
                activation_time: 1_600_000_000,
                is_active: false,
            }],
        )
        .await;

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report), None)
        .await;
    Assert::transaction_error(&result, ErrorCode::ConfigDeactivated);
}

#[tokio::test]
async fn test_legacy_verify_with_max_configs() {
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

    let (first_report, first_signers) = generate_report_with_signers::<V3Report>(16, 6, Some(1_600_000_000), None);
    let last_activation_time = 1_600_000_000 + MAX_NUMBER_OF_DON_CONFIGS as u32;
    let (last_report, last_signers) = generate_report_with_signers::<V3Report>(16, 6, Some(last_activation_time), None);

    let mut configs = vec![legacy_config(first_signers, 5, 1_600_000_000)];
    for i in 1..MAX_NUMBER_OF_DON_CONFIGS - 1 {
        let (_, signers) = generate_report_with_signers::<V3Report>(16, 6, None, None);
        configs.push(legacy_config(signers, 5, 1_600_000_000 + i as u32));
    }
    configs.push(legacy_config(last_signers, 5, last_activation_time));

    verifier_client
        .set_legacy_don_configs(&mut environment_context, &configs)
        .await;

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&first_report), None)
        .await;
    Assert::transaction_ok(&result);

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&last_report), None)
        .await;
    Assert::transaction_ok(&result);
}
