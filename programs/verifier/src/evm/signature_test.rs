use super::encoding::Encoder;
use super::signature::{ecrecover, is_zero_address};
use anchor_lang::solana_program::keccak::hash as keccak256;
use test_utils::report::{generate_report_with_signers_and_digest, get_signers, test_config_digest, V3Report};

// Signs a report with the given signers and returns the report with the hash the signatures cover
fn signed_report(signers: &[test_utils::report::Signer]) -> (Vec<u8>, [u8; 32]) {
    let (report, _) = generate_report_with_signers_and_digest::<V3Report>(
        test_config_digest(1),
        signers.len(),
        signers.len(),
        None,
        Some(signers.to_vec()),
    );

    let parsed = Encoder::parse_signed_report(&report).unwrap();
    let hash = keccak256(
        &[
            &keccak256(parsed.report_data).to_bytes()[..],
            &parsed.report_context[0][..],
            &parsed.report_context[1][..],
            &parsed.report_context[2][..],
        ]
        .concat(),
    )
    .to_bytes();

    (report, hash)
}

#[test]
fn test_ecrecover_returns_signer_address() {
    let signers = get_signers(4);
    let (report, hash) = signed_report(&signers);
    let parsed = Encoder::parse_signed_report(&report).unwrap();

    for (i, signer) in signers.iter().enumerate() {
        let address = ecrecover(&hash, &parsed.rs[i], &parsed.ss[i], parsed.raw_vs[i]).unwrap();
        assert_eq!(address, signer.signer_address);
    }
}

#[test]
fn test_ecrecover_accepts_v_as_27_or_28() {
    let signers = get_signers(4);
    let (report, hash) = signed_report(&signers);
    let parsed = Encoder::parse_signed_report(&report).unwrap();

    for (i, signer) in signers.iter().enumerate() {
        // Reports encode v as the recovery id (0 or 1), Ethereum style v is 27 or 28
        let v = parsed.raw_vs[i];
        assert!(v <= 1);
        let address = ecrecover(&hash, &parsed.rs[i], &parsed.ss[i], v + 27).unwrap();
        assert_eq!(address, signer.signer_address);
    }
}

#[test]
fn test_ecrecover_with_wrong_v_does_not_return_signer() {
    let signers = get_signers(1);
    let (report, hash) = signed_report(&signers);
    let parsed = Encoder::parse_signed_report(&report).unwrap();

    let flipped_v = 1 - parsed.raw_vs[0];
    if let Ok(address) = ecrecover(&hash, &parsed.rs[0], &parsed.ss[0], flipped_v) {
        assert_ne!(address, signers[0].signer_address);
    }
}

#[test]
fn test_ecrecover_fails_on_invalid_recovery_id() {
    let signers = get_signers(1);
    let (report, hash) = signed_report(&signers);
    let parsed = Encoder::parse_signed_report(&report).unwrap();

    for v in [4u8, 26, 31, 255] {
        assert!(ecrecover(&hash, &parsed.rs[0], &parsed.ss[0], v).is_err(), "v = {v}");
    }
}

#[test]
fn test_ecrecover_with_wrong_hash_does_not_return_signer() {
    let signers = get_signers(1);
    let (report, mut hash) = signed_report(&signers);
    let parsed = Encoder::parse_signed_report(&report).unwrap();
    hash[0] ^= 0xff;

    if let Ok(address) = ecrecover(&hash, &parsed.rs[0], &parsed.ss[0], parsed.raw_vs[0]) {
        assert_ne!(address, signers[0].signer_address);
    }
}

#[test]
fn test_ecrecover_fails_on_invalid_signature() {
    let hash = [1u8; 32];

    // r and s must be in the range [1, n - 1] of the secp256k1 curve
    assert!(ecrecover(&hash, &[0u8; 32], &[1u8; 32], 0).is_err());
    assert!(ecrecover(&hash, &[1u8; 32], &[0u8; 32], 0).is_err());
    assert!(ecrecover(&hash, &[0xffu8; 32], &[0xffu8; 32], 0).is_err());
}

#[test]
fn test_is_zero_address() {
    assert!(is_zero_address(&[0u8; 20]));

    let mut address = [0u8; 20];
    address[19] = 1;
    assert!(!is_zero_address(&address));

    let mut address = [0u8; 20];
    address[0] = 1;
    assert!(!is_zero_address(&address));
}
