use crate::common::fixtures::{
    fixture_signers, FIXTURE_DIGEST, FIXTURE_F, FIXTURE_REPORT, FIXTURE_REPORT_DATA,
};
use crate::common::test_setup::{VerifierTestSetup, VerifierTestSetupBuilder};
use access_controller::AccessController;
use anchor_lang::{Discriminator, ToAccountMetas};
use chainlink_solana_data_streams::VerifierInstructions;
use hex::encode as hex_encode;
use hex_literal::hex;
use solana_program::pubkey::Pubkey;
use solana_program::system_instruction;
use solana_program_test::tokio;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use std::mem::size_of;
use anchor_lang::error::ErrorCode::AccountOwnedByWrongProgram;
use test_utils::assert::Assert;
use test_utils::client::LegacyDonConfig;
use test_utils::environment_context_operations::EnvironmentContextOperations;
use test_utils::report::{
    generate_report_with_signers_and_digest, get_signers, test_config_digest, DummyReport,
    V3Report,
};
use verifier::errors::ErrorCode;
use verifier::errors::ErrorCode::{InvalidAccessController, Unauthorized};
use verifier::state::VerifierAccount;
use verifier::util::Compressor;

pub mod common;

#[tokio::test]
async fn test_verify_report() {
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

    let compressed_report = Compressor::compress(&FIXTURE_REPORT);

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

    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;

    Assert::transaction_ok(&result);

    let metadata = result
        .expect("Transaction result should be present")
        .metadata
        .expect("Metadata should be present");

    // The digest config was used, not the legacy fallback
    assert!(
        !metadata.log_messages.iter().any(|l| l.contains("legacy config fallback")),
        "Verify should not use the legacy fallback when the config PDA exists"
    );

    let data = metadata
        .return_data
        .expect("Return data should be present")
        .data;

    assert_eq!(hex_encode(&data), FIXTURE_REPORT_DATA);
}

#[tokio::test]
async fn test_verify_with_sdk_instruction() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        access_controller_account_address,
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

    // Build the instruction exactly as integrators do with the published SDK
    let instruction = VerifierInstructions::verify(
        &verifier::ID,
        &VerifierInstructions::get_verifier_config_pda(&verifier::ID),
        &access_controller_account_address.unwrap(),
        &user.pubkey(),
        &VerifierInstructions::get_config_pda(&FIXTURE_REPORT, &verifier::ID),
        Compressor::compress(&FIXTURE_REPORT),
    );

    let result = EnvironmentContextOperations::send_transaction(
        &mut environment_context,
        &[instruction],
        Some(&user.pubkey()),
        &[&user],
    )
    .await;
    Assert::transaction_ok(&result);

    let data = result
        .unwrap()
        .metadata
        .unwrap()
        .return_data
        .expect("Return data should be present")
        .data;
    assert_eq!(hex_encode(&data), FIXTURE_REPORT_DATA);
}

#[test]
fn test_verify_interface_matches_sdk() {
    // Instruction discriminator
    assert_eq!(
        chainlink_solana_data_streams::discriminator::VERIFY,
        verifier::instruction::Verify::DISCRIMINATOR
    );

    // Account order, signer and writable flags
    let verifier_account = Pubkey::new_unique();
    let access_controller = Pubkey::new_unique();
    let user = Pubkey::new_unique();
    let config_account = Pubkey::new_unique();

    let program_metas = verifier::accounts::VerifyContext {
        verifier_account,
        access_controller,
        user,
        config_account,
    }
    .to_account_metas(None);

    let sdk_metas = VerifierInstructions::verify(
        &verifier::ID,
        &verifier_account,
        &access_controller,
        &user,
        &config_account,
        vec![],
    )
    .accounts;

    assert_eq!(program_metas.len(), sdk_metas.len());
    for (program_meta, sdk_meta) in program_metas.iter().zip(sdk_metas.iter()) {
        assert_eq!(program_meta.pubkey, sdk_meta.pubkey);
        assert_eq!(program_meta.is_signer, sdk_meta.is_signer);
        assert_eq!(program_meta.is_writable, sdk_meta.is_writable);
    }
}

#[tokio::test]
async fn test_fail_to_verify_report_if_signer_not_in_config() {
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

    let compressed_report = Compressor::compress(&FIXTURE_REPORT);

    // signers list (as configured when example report was generated)
    // but missing signer `29679cD77AAce065B885b190368f04fDD7E587AD`
    let signers: Vec<[u8; 20]> = vec![
        hex!("38C7EA2f6b878509f3e2d0bbE9adF328e1Df2f6C"),
        hex!("a669f0bE9F92e3fe5Eb7b28d1852dFf84C7516Cc"),
        hex!("8735F9dd83c0b03571b39Fe9FfbB05e02bc08c28"),
        hex!("a75F02C31207087dc849007ef9221B11eE6CB559")
    ];

    let result = verifier_client
        .set_config(&mut environment_context, &user, FIXTURE_DIGEST, signers, FIXTURE_F)
        .await;
    Assert::transaction_ok(&result);

    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;

    Assert::transaction_error(&result, ErrorCode::BadVerification);
}

#[tokio::test]
async fn test_can_verify_with_non_owner() {
    let non_owner = Keypair::new();

    let VerifierTestSetup {
        mut environment_context,
        verifier_client,
        access_controller_client,
        user,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .add_user(non_owner.pubkey())
        .build()
        .await;

    let result = access_controller_client
        .unwrap()
        .add_access(
            &mut environment_context,
            &user,
            non_owner.pubkey()
            )
            .await;
    Assert::transaction_ok(&result);

    let digest = test_config_digest(1);
    let (report, signers) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, None);
    let compressed_report = Compressor::compress(&report);

    // Set up the configuration
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers, 5)
        .await;
    Assert::transaction_ok(&result);

    // Verify with non-owner
    let result = verifier_client
        .verify(
            &mut environment_context,
            &non_owner,
            compressed_report,
            None,
        )
        .await;

    Assert::transaction_ok(&result);
}


#[tokio::test]
async fn test_can_verify_without_access() {
    let unauthorized_user = Keypair::new();

    let VerifierTestSetup {
        mut environment_context,
        verifier_client,
        user,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .add_user(unauthorized_user.pubkey())
        .build()
        .await;

    let digest = test_config_digest(1);
    let (report, signers) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, None);
    let compressed_report = Compressor::compress(&report);

    // Set up the configuration
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers, 5)
        .await;
    Assert::transaction_ok(&result);

    // Verify with unauthorized_user
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report,
            None,
        )
        .await;

    Assert::transaction_error(&result, ErrorCode::Unauthorized);
}

#[tokio::test]
async fn test_verify_fails_with_invalid_access_controller() {
    let dummy_access_controller = Pubkey::new_unique();
    let VerifierTestSetup {
        mut environment_context,
        user,
        mut verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .add_zero_copy_account(access_controller::ID, dummy_access_controller, size_of::<AccessController>(), Some(AccessController::discriminator()))
        .build()
        .await;

    // Mock that user will attempt to pass an invalid access controller
    verifier_client.access_controller_data_account_override(Some(dummy_access_controller));

    let verifier_account: VerifierAccount = verifier_client
        .read_verifier_account(&mut environment_context)
        .await
        .unwrap();

    // Verifier account should have a valid access controller
    assert_ne!(verifier_account.verifier_account_config.access_controller, Pubkey::default());
    assert_ne!(verifier_account.verifier_account_config.access_controller, dummy_access_controller);

    let digest = test_config_digest(1);
    let (report, _) = generate_report_with_signers_and_digest::<DummyReport>(digest, 16, 6, None, None);
    let compressed_report = Compressor::compress(&report);

    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;


    Assert::transaction_error(&result, InvalidAccessController);
}

#[tokio::test]
async fn test_incorrect_config_account_fails_verification() {
    let dummy_config_account = Pubkey::new_unique();
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
    let (report, _) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, None);
    let compressed_report = Compressor::compress(&report);

    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, Some(dummy_config_account))
        .await;

    Assert::transaction_error(&result, ErrorCode::InvalidConfigAccount);
}

#[tokio::test]
async fn test_config_account_of_another_digest_fails_verification() {
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

    let digest_a = test_config_digest(1);
    let digest_b = test_config_digest(2);

    let (report_a, signers_a) = generate_report_with_signers_and_digest::<V3Report>(digest_a, 16, 6, None, None);
    let (_, signers_b) = generate_report_with_signers_and_digest::<V3Report>(digest_b, 16, 6, None, None);

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest_a, signers_a, 5)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest_b, signers_b, 5)
            .await,
    );

    // Passing an existing config account that does not belong to the report's digest is rejected
    let result = verifier_client
        .verify(
            &mut environment_context,
            &user,
            Compressor::compress(&report_a),
            Some(verifier_client.config_pda(&digest_b)),
        )
        .await;

    Assert::transaction_error(&result, ErrorCode::InvalidConfigAccount);
}

#[tokio::test]
async fn test_verify_reports_from_multiple_dons() {
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

    // e.g. DON 3: 4 signers, f = 1
    let digest_a = test_config_digest(3);
    let signers_a = get_signers(4);
    // e.g. DON 4: 16 signers, f = 5
    let digest_b = test_config_digest(4);
    let signers_b = get_signers(16);

    Assert::transaction_ok(
        &verifier_client
            .set_config(
                &mut environment_context,
                &user,
                digest_a,
                signers_a.iter().map(|s| s.signer_address).collect(),
                1,
            )
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .set_config(
                &mut environment_context,
                &user,
                digest_b,
                signers_b.iter().map(|s| s.signer_address).collect(),
                5,
            )
            .await,
    );

    // Reports from both DONs verify, regardless of report timestamps or the order configs were set
    let (report_a, _) = generate_report_with_signers_and_digest::<V3Report>(digest_a, 4, 2, Some(1), Some(signers_a.clone()));
    let (report_b, _) = generate_report_with_signers_and_digest::<V3Report>(digest_b, 16, 6, None, Some(signers_b.clone()));

    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&report_a), None)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&report_b), None)
            .await,
    );

    // A report signed by DON A's signers for DON B's digest does not verify
    let (cross_report, _) = generate_report_with_signers_and_digest::<V3Report>(digest_b, 4, 4, None, Some(signers_a));
    Assert::transaction_error(
        &verifier_client
            .verify(&mut environment_context, &user, Compressor::compress(&cross_report), None)
            .await,
        ErrorCode::BadVerification,
    );
}

#[tokio::test]
async fn test_deactivated_config_does_not_use_legacy_fallback() {
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
    let (report, signers) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, None);

    // A matching legacy config exists and the fallback is enabled
    verifier_client
        .set_legacy_don_configs(
            &mut environment_context,
            &[LegacyDonConfig {
                signers: signers.clone(),
                f: 5,
                activation_time: 1_600_000_000,
                is_active: true,
            }],
        )
        .await;

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest, signers, 5)
            .await,
    );
    Assert::transaction_ok(
        &verifier_client
            .deactivate_config(&mut environment_context, &user, digest)
            .await,
    );

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report), None)
        .await;

    Assert::transaction_error(&result, ErrorCode::ConfigDeactivated);
}

#[tokio::test]
async fn test_verify_unregistered_digest_fails_when_legacy_fallback_disabled() {
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

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&FIXTURE_REPORT), None)
        .await;

    Assert::transaction_error(&result, ErrorCode::DigestNotSet);
}

#[tokio::test]
async fn test_verify_with_prefunded_config_account() {
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

    // Anyone can send lamports to the config PDA address before the config is set
    let config_account = verifier_client.config_pda(&FIXTURE_DIGEST);
    let transfer = system_instruction::transfer(&user.pubkey(), &config_account, 1_000_000);
    Assert::transaction_ok(
        &EnvironmentContextOperations::send_transaction(
            &mut environment_context,
            &[transfer],
            Some(&user.pubkey()),
            &[&user],
        )
        .await,
    );

    // A system owned account is not treated as a config
    let compressed_report = Compressor::compress(&FIXTURE_REPORT);
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report.clone(), None)
        .await;
    Assert::transaction_error(&result, ErrorCode::DigestNotSet);

    // The config can still be set on the prefunded address
    Assert::transaction_ok(
        &verifier_client
            .set_config(
                &mut environment_context,
                &user,
                FIXTURE_DIGEST,
                fixture_signers(),
                FIXTURE_F,
            )
            .await,
    );

    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;
    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_fail_to_verify_report_if_not_enough_signers() {
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

    // Generate report with only 1 active signer
    let digest = test_config_digest(1);
    let (report, signers) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 1, None, None);
    let f = 5; // Requires more than 1 signer

    // Set the config first
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers, f)
        .await;

    Assert::transaction_ok(&result);

    // Try to verify with insufficient signers
    let compressed_report = Compressor::compress(&report);
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;

    Assert::transaction_error(&result, ErrorCode::BadVerification);
}

#[tokio::test]
async fn test_fail_to_verify_report_if_no_signers() {
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

    // Generate report with 0 active signers
    let digest = test_config_digest(1);
    let (report, signers) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 0, None, None);
    let f = 5;

    // Set the config first
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers, f)
        .await;

    Assert::transaction_ok(&result);

    // Try to verify with no signers
    let compressed_report = Compressor::compress(&report);
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;

    Assert::transaction_error(&result, ErrorCode::NoSigners);
}

#[tokio::test]
async fn test_fail_to_verify_report_if_mismatched_signatures() {
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
    let (mut report, signers) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, None);

    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest, signers, 5)
            .await,
    );

    // Shorten the ss array by one, the offset of ss is the 6th word of the report
    let ss_offset = u64::from_be_bytes(report[5 * 32 + 24..6 * 32].try_into().unwrap()) as usize;
    report[ss_offset + 31] -= 1;

    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&report), None)
        .await;

    Assert::transaction_error(&result, ErrorCode::MismatchedSignatures);
}

#[tokio::test]
async fn test_fail_to_verify_report_if_dup_signers() {
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

    // Generate signers for the config
    let signers = get_signers(16);
    let signer_pub_keys = signers.iter().map(|s| s.signer_address).collect();

    // Add a duplicate signer to generate a malformed report
    let mut signers_with_duplicate = signers[0..6].to_vec();
    signers_with_duplicate.push(signers[0].clone());

    // Generate report with duplicate signers
    let digest = test_config_digest(1);
    let (report, _) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 7, None, Some(signers_with_duplicate));
    let f = 5;

    // Set the config first
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signer_pub_keys, f)
        .await;

    Assert::transaction_ok(&result);

    // Try to verify with duplicate signers
    let compressed_report = Compressor::compress(&report);
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;

    Assert::transaction_error(&result, ErrorCode::BadVerification);
}

#[tokio::test]
async fn test_verify_with_too_many_signers() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        // 7 signatures is close to the default 200k CU limit, and the on-chain config PDA
        // derivation cost varies with the digest
        .with_compute_max_units(400_000)
        .build()
        .await;

    // Generate report with 7 signatures, more than the f + 1 required
    let digest = test_config_digest(1);
    let (report, signer_pub_keys) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 7, None, None);
    let f = 5;

    // Set the config
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signer_pub_keys, f)
        .await;

    Assert::transaction_ok(&result);

    // More than f + 1 signatures still verifies
    let compressed_report = Compressor::compress(&report);
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;

    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_can_verify_non_standard_reports() {
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

    // Generate report with signers
    let digest = test_config_digest(1);
    let (report, signers) = generate_report_with_signers_and_digest::<DummyReport>(digest, 16, 6, None, None);
    let compressed_report = Compressor::compress(&report);

    // Set up the configuration
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers, 5)
        .await;
    Assert::transaction_ok(&result);

    // Verify the report
    let result = verifier_client
        .verify(
            &mut environment_context,
            &user,
            compressed_report,
            None,
        )
        .await;

    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_verify_with_max_signers() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .with_compute_max_units(2_000_000)
        .build()
        .await;

    // Generate report with 31 signers
    let digest = test_config_digest(1);
    let (report, signers) = generate_report_with_signers_and_digest::<V3Report>(digest, 31, 11, None, None);
    let f = 10;

    // Set the config first
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers, f)
        .await;

    Assert::transaction_ok(&result);

    // Verify report with max signers
    let compressed_report = Compressor::compress(&report);
    let result = verifier_client
        .verify(&mut environment_context, &user, compressed_report, None)
        .await;

    Assert::transaction_ok(&result);
}

#[tokio::test]
async fn test_verify_compute_units_digest_config_vs_legacy() {
    let VerifierTestSetup {
        mut environment_context,
        user,
        verifier_client,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .with_compute_max_units(2_000_000)
        .build()
        .await;

    let signers = get_signers(16);
    let signer_addresses: Vec<[u8; 20]> = signers.iter().map(|s| s.signer_address).collect();

    // Legacy path: report digest has no config PDA
    verifier_client
        .set_legacy_don_configs(
            &mut environment_context,
            &[LegacyDonConfig {
                signers: signer_addresses.clone(),
                f: 5,
                activation_time: 1_600_000_000,
                is_active: true,
            }],
        )
        .await;
    let (legacy_report, _) = generate_report_with_signers_and_digest::<V3Report>(test_config_digest(1), 16, 6, None, Some(signers.clone()));
    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&legacy_report), None)
        .await;
    Assert::transaction_ok(&result);
    let legacy_units = result.unwrap().metadata.unwrap().compute_units_consumed;

    // Digest path: same signers and signature count
    let digest = test_config_digest(2);
    Assert::transaction_ok(
        &verifier_client
            .set_config(&mut environment_context, &user, digest, signer_addresses, 5)
            .await,
    );
    let (digest_report, _) = generate_report_with_signers_and_digest::<V3Report>(digest, 16, 6, None, Some(signers));
    let result = verifier_client
        .verify(&mut environment_context, &user, Compressor::compress(&digest_report), None)
        .await;
    Assert::transaction_ok(&result);
    let digest_units = result.unwrap().metadata.unwrap().compute_units_consumed;

    println!("verify compute units: legacy = {legacy_units}, digest config = {digest_units}");

    // Integrators size their compute budget for today's verify, so the digest path must not cost
    // noticeably more
    assert!(
        digest_units <= legacy_units + 5_000,
        "digest config verify ({digest_units} CU) costs more than legacy verify ({legacy_units} CU)"
    );
}

#[tokio::test]
async fn test_verify_with_disabled_access_controller() {
    let unauthorized_user = Keypair::new();
    let authorized_user = Keypair::new();

    let VerifierTestSetup {
        mut environment_context,
        user,
        mut verifier_client,
        access_controller_client,
        access_controller_account_address,
        ..
    } = VerifierTestSetupBuilder::new()
        .program_name("verifier")
        .program_id(verifier::ID)
        .access_controller(access_controller::ID)
        .add_user(unauthorized_user.pubkey())
        .add_user(authorized_user.pubkey())
        .build()
        .await;

    let verifier_account: VerifierAccount = verifier_client
        .read_verifier_account(&mut environment_context)
        .await
        .unwrap();

    // Ensure initial state is access controller enabled with valid accounts
    assert_ne!(access_controller_account_address.unwrap(), Pubkey::default());
    assert_ne!(verifier_account.verifier_account_config.access_controller, Pubkey::default());

    let digest = test_config_digest(1);
    let (report, signers) = generate_report_with_signers_and_digest::<DummyReport>(digest, 16, 6, None, None);
    let compressed_report = Compressor::compress(&report);

    // Set up the configuration
    let result = verifier_client
        .set_config(&mut environment_context, &user, digest, signers, 5)
        .await;
    Assert::transaction_ok(&result);

    let result = access_controller_client
        .unwrap()
        .add_access(
            &mut environment_context,
            &user,
            authorized_user.pubkey()
        )
        .await;
    Assert::transaction_ok(&result);

    // Authorized user can verify
    let result = verifier_client
        .verify(
            &mut environment_context,
            &authorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_ok(&result);

    // Non-access user cannot verify when access controller is enabled
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_error(&result, Unauthorized);

    // Disable access controller
    let result = verifier_client
        .set_access_controller(&mut environment_context, &user, None)
        .await;
    Assert::transaction_ok(&result);

    let verifier_account: VerifierAccount = verifier_client
        .read_verifier_account(&mut environment_context)
        .await
        .unwrap();

    // Ensure access controller is disabled in account
    assert_eq!(
        verifier_account.verifier_account_config.access_controller,
        Pubkey::default(),
        "Access controller should be disabled in account (should equal Pubkey::default())"
    );

    // When access controller is set we can verify using an existing access controller account even when disabled
    assert_eq!(
        verifier_client.access_controller_data_account.unwrap(),
        access_controller_account_address.unwrap(),
        "Verifier client should use the access controller account address for interaction"
    );

    // Authorized user can verify when access controller is disabled
    let result = verifier_client
        .verify(
            &mut environment_context,
            &authorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_ok(&result);

    // Non-access user can verify when access controller is disabled
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_ok(&result);

    // Transactions with `Pubkey::default()` as AC account will fail during Anchor checks
    verifier_client.access_controller_data_account_override(Some(Pubkey::default()));
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_error(&result, AccountOwnedByWrongProgram);

    let result = verifier_client
        .verify(
            &mut environment_context,
            &authorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_error(&result, AccountOwnedByWrongProgram);

    // Set access controller back to valid account
    verifier_client.access_controller_data_account_override(access_controller_account_address);
    assert_eq!(
        verifier_client.access_controller_data_account.unwrap(),
        access_controller_account_address.unwrap(),
        "Verifier client should use the access controller account address for interaction"
    );
    let result = verifier_client
        .set_access_controller(&mut environment_context, &user, access_controller_account_address)
        .await;
    Assert::transaction_ok(&result);

    // Non-access user cannot verify when access controller is re-enabled
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_error(&result, Unauthorized);

    // Authorized user can verify when access controller is re-enabled
    let result = verifier_client
        .verify(
            &mut environment_context,
            &authorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_ok(&result);

    // User tries to pass with Any other account will fail

    verifier_client.access_controller_data_account_override(Some(Pubkey::new_unique()));
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_error(&result, AccountOwnedByWrongProgram);

    verifier_client.access_controller_data_account_override(Some(verifier::ID));
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_error(&result, AccountOwnedByWrongProgram);

    verifier_client.access_controller_data_account_override(Some(verifier_client.data_account));
    let result = verifier_client
        .verify(
            &mut environment_context,
            &unauthorized_user,
            compressed_report.clone(),
            None,
        )
        .await;
    Assert::transaction_error(&result, AccountOwnedByWrongProgram);
}
