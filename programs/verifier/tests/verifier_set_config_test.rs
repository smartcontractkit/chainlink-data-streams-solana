use crate::common::test_setup::{VerifierTestSetup, VerifierTestSetupBuilder};
use anchor_lang::Space;
use solana_program::pubkey::Pubkey;
use solana_program::rent::Rent;
use solana_program_test::tokio;
use solana_sdk::signature::{Keypair, Signer};
use test_utils::assert::Assert;
use test_utils::environment_context_operations::EnvironmentContextOperations;
use test_utils::report::{generate_report_with_signers, test_config_digest, V3Report};
use verifier::common::MAX_NUMBER_OF_ORACLES;
use verifier::errors::ErrorCode;
use verifier::events::ConfigSet;
use verifier::state::{DigestConfig, DIGEST_CONFIG_VERSION};
use verifier::util::LogParser;

pub mod common;

fn test_signers(count: u8) -> Vec<[u8; 20]> {
    (0..count)
        .map(|i| {
            let mut signer = [0u8; 20];
            signer[0] = i + 1;
            signer
        })
        .collect()
}

#[tokio::test]
async fn test_set_config() {
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
    // Unsorted signers are stored sorted
    let mut signers = test_signers(16);
    signers.reverse();
    let f: u8 = 5;

    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers.clone(), f)
        .await;
    Assert::transaction_ok(&result);

    let mut sorted_signers = signers.clone();
    sorted_signers.sort_unstable();

    // Config PDA state
    let config = verifier_client
        .read_digest_config(&mut environment_context, digest)
        .await
        .expect("Config account should exist");

    let (expected_address, expected_bump) =
        Pubkey::find_program_address(&[digest.as_ref()], &verifier::ID);
    assert_eq!(verifier_client.config_pda(&digest), expected_address);
    assert_eq!(config.version, DIGEST_CONFIG_VERSION);
    assert_eq!(config.config_digest, digest);
    assert_eq!(config.f, f);
    assert!(config.is_active);
    assert_eq!(config.bump, expected_bump);
    assert_eq!(config.signers, sorted_signers);

    // Every config account has the same size, large enough for the max number of signers
    let account = EnvironmentContextOperations::get_account(&mut environment_context, expected_address)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.owner, verifier::ID);
    assert_eq!(account.data.len(), 8 + DigestConfig::INIT_SPACE);
    assert_eq!(account.lamports, Rent::default().minimum_balance(8 + DigestConfig::INIT_SPACE));

    // Event
    let event: Option<ConfigSet> = LogParser::parse_logs(result.unwrap().metadata.unwrap().log_messages);
    let event = event.expect("ConfigSet event should be emitted");
    assert_eq!(event.config_digest, digest);
    assert_eq!(event.signers, sorted_signers);
    assert_eq!(event.f, f);

    // Legacy configs are not touched
    let verifier_account = verifier_client
        .read_verifier_account(&mut environment_context)
        .await
        .unwrap();
    assert!(verifier_account.don_configs.is_empty());
}

#[tokio::test]
async fn test_set_config_with_max_signers() {
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
    let signers = test_signers(MAX_NUMBER_OF_ORACLES);

    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers.clone(), 10)
        .await;
    Assert::transaction_ok(&result);

    let config = verifier_client
        .read_digest_config(&mut environment_context, digest)
        .await
        .unwrap();
    assert_eq!(config.signers.len(), MAX_NUMBER_OF_ORACLES as usize);
}

#[tokio::test]
async fn test_set_config_with_non_owner() {
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

    let digest = test_config_digest(1);
    let result = verifier_client
        .set_config(&mut environment_context, &non_owner, digest, test_signers(4), 1)
        .await;

    Assert::transaction_error(&result, ErrorCode::Unauthorized);

    assert!(verifier_client
        .read_digest_config(&mut environment_context, digest)
        .await
        .is_none());
}

#[tokio::test]
async fn test_set_config_fails_when_digest_already_set() {
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

    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, test_signers(4), 1)
        .await;
    Assert::transaction_ok(&result);

    // Same digest, same config
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, test_signers(4), 1)
        .await;
    Assert::transaction_error(&result, ErrorCode::ConfigDigestAlreadySet);

    // Same digest, different config
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, test_signers(16), 5)
        .await;
    Assert::transaction_error(&result, ErrorCode::ConfigDigestAlreadySet);

    // The original config is unchanged
    let config = verifier_client
        .read_digest_config(&mut environment_context, digest)
        .await
        .unwrap();
    assert_eq!(config.f, 1);
    assert_eq!(config.signers, test_signers(4));
}

#[tokio::test]
async fn test_set_config_same_signers_for_multiple_digests() {
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

    // e.g. blue/green configs of the same DON
    for n in 1..=3 {
        let result = verifier_client
            .set_config(&mut environment_context, &user, test_config_digest(n), test_signers(4), 1)
            .await;
        Assert::transaction_ok(&result);
    }

    for n in 1..=3 {
        assert!(verifier_client
            .read_digest_config(&mut environment_context, test_config_digest(n))
            .await
            .is_some());
    }
}

#[tokio::test]
async fn test_set_config_fails_on_empty_digest() {
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
        .set_config(&mut environment_context, &user, [0u8; 32], test_signers(4), 1)
        .await;

    Assert::transaction_error(&result, ErrorCode::DigestEmpty);
}

#[tokio::test]
async fn test_set_config_fails_on_too_many_signers() {
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

    // 32 is too many
    let result = verifier_client
        .set_config(&mut environment_context, &user, test_config_digest(1), test_signers(32), 2)
        .await;

    Assert::transaction_error(&result, ErrorCode::ExcessSigners);
}

#[tokio::test]
async fn test_set_config_fails_on_too_high_f() {
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

    // 16 signers requires f <= 5
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, test_signers(16), 6)
        .await;
    Assert::transaction_error(&result, ErrorCode::InsufficientSigners);

    // internal calc (86 * 3 = 258) > u8 (256) would overflow the u8
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, test_signers(10), 86)
        .await;
    Assert::transaction_error(&result, ErrorCode::InsufficientSigners);
}

#[tokio::test]
async fn test_set_config_fails_on_zero_fault_tolerance() {
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
        .set_config(&mut environment_context, &user, test_config_digest(1), test_signers(15), 0)
        .await;

    Assert::transaction_error(&result, ErrorCode::FaultToleranceMustBePositive);
}

#[tokio::test]
async fn test_set_config_fails_on_duplicate_signer() {
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

    let signers: Vec<[u8; 20]> = vec![
        [0x12; 20], // Signer 1
        [0x34; 20], // Signer 2
        [0x56; 20], // Signer 3
        [0x56; 20], // Signer 3 -- duplicate
    ];

    let result = verifier_client
        .set_config(&mut environment_context, &user, test_config_digest(1), signers, 1)
        .await;

    Assert::transaction_error(&result, ErrorCode::NonUniqueSignatures);
}

#[tokio::test]
async fn test_set_config_fails_on_zero_address_signer() {
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

    let (_, mut signers) = generate_report_with_signers::<V3Report>(16, 6, None, None);
    let f = (signers.len() / 3) as u8;

    // Add zero address
    signers.push([0u8; 20]);

    let result = verifier_client
        .set_config(&mut environment_context, &user, test_config_digest(1), signers, f)
        .await;

    Assert::transaction_error(&result, ErrorCode::ZeroAddress);
}
