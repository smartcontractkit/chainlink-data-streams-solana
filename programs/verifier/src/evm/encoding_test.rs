use super::encoding::Encoder;
use crate::errors::ErrorCode;
use test_utils::report::{
    generate_report_with_signers_and_digest, test_config_digest, Report, V3Report,
};

const WORD_SIZE: usize = 32;
const TIMESTAMP: u32 = 1_750_000_000;

fn signed_report(signers_per_report: usize) -> Vec<u8> {
    let (report, _) = generate_report_with_signers_and_digest::<V3Report>(
        test_config_digest(1),
        4,
        signers_per_report,
        Some(TIMESTAMP),
        None,
    );
    report
}

fn word(data: &[u8], index: usize) -> usize {
    u64::from_be_bytes(data[index * WORD_SIZE + 24..(index + 1) * WORD_SIZE].try_into().unwrap()) as usize
}

fn set_word(data: &mut [u8], byte_offset: usize, value: u64) {
    data[byte_offset..byte_offset + WORD_SIZE].fill(0);
    data[byte_offset + 24..byte_offset + WORD_SIZE].copy_from_slice(&value.to_be_bytes());
}

fn assert_bad_verification<T: std::fmt::Debug>(result: Result<T, ErrorCode>) {
    assert!(
        matches!(result, Err(ErrorCode::BadVerification)),
        "expected BadVerification, got {:?}",
        result
    );
}

// Head of the signed report: report_context (3 words), then offsets of report_data, rs, ss, then raw_vs
const REPORT_DATA_OFFSET_WORD: usize = 3;
const RS_OFFSET_WORD: usize = 4;
const SS_OFFSET_WORD: usize = 5;

#[test]
fn test_parse_signed_report() {
    let report = signed_report(3);
    let parsed = Encoder::parse_signed_report(&report).unwrap();

    assert_eq!(parsed.report_context[0], test_config_digest(1));
    assert_eq!(parsed.report_context[1], [0u8; 32]);
    assert_eq!(parsed.report_context[2], [0u8; 32]);
    assert_eq!(parsed.report_data, V3Report::dummy(Some(TIMESTAMP)).encode().as_slice());
    assert_eq!(parsed.rs.len(), 3);
    assert_eq!(parsed.ss.len(), 3);
    assert!(parsed.raw_vs[..3].iter().all(|v| *v <= 1));
    assert!(parsed.raw_vs[3..].iter().all(|v| *v == 0));
}

#[test]
fn test_parse_signed_report_without_signatures() {
    let report = signed_report(0);
    let parsed = Encoder::parse_signed_report(&report).unwrap();

    assert!(parsed.rs.is_empty());
    assert!(parsed.ss.is_empty());
}

#[test]
fn test_parse_report_details() {
    let report_data = V3Report::dummy(Some(TIMESTAMP)).encode();
    let details = Encoder::parse_report_details_from_report(&report_data).unwrap();

    assert_eq!(details.feed_id, &report_data[0..32]);
    assert_eq!(details.feed_id[0..2], [0x00, 0x03]);
    assert_eq!(details.report_timestamp, TIMESTAMP);
}

#[test]
fn test_parse_report_details_needs_96_bytes() {
    let report_data = V3Report::dummy(Some(TIMESTAMP)).encode();

    assert!(Encoder::parse_report_details_from_report(&report_data[..96]).is_ok());
    assert_bad_verification(Encoder::parse_report_details_from_report(&report_data[..95]));
    assert_bad_verification(Encoder::parse_report_details_from_report(&report_data[..31]));
    assert_bad_verification(Encoder::parse_report_details_from_report(&[]));
}

#[test]
fn test_parse_fails_on_short_input() {
    assert_bad_verification(Encoder::parse_signed_report(&[]));
    assert_bad_verification(Encoder::parse_signed_report(&[0u8; 7 * WORD_SIZE - 1]));
}

#[test]
fn test_parse_fails_on_truncated_report() {
    let report = signed_report(3);

    // Every truncation of a valid report must fail cleanly
    for len in 0..report.len() {
        assert_bad_verification(Encoder::parse_signed_report(&report[..len]));
    }
}

#[test]
fn test_parse_fails_on_offset_out_of_bounds() {
    for offset_word in [REPORT_DATA_OFFSET_WORD, RS_OFFSET_WORD, SS_OFFSET_WORD] {
        let mut report = signed_report(3);
        let len = report.len() as u64;

        // Points at the last word, so there is no room for the data after the length
        set_word(&mut report, offset_word * WORD_SIZE, len - WORD_SIZE as u64 + 1);
        assert_bad_verification(Encoder::parse_signed_report(&report));

        set_word(&mut report, offset_word * WORD_SIZE, len);
        assert_bad_verification(Encoder::parse_signed_report(&report));

        // Would overflow when adding the word size
        set_word(&mut report, offset_word * WORD_SIZE, u64::MAX);
        assert_bad_verification(Encoder::parse_signed_report(&report));
    }
}

#[test]
fn test_parse_fails_on_length_out_of_bounds() {
    for offset_word in [REPORT_DATA_OFFSET_WORD, RS_OFFSET_WORD, SS_OFFSET_WORD] {
        let mut report = signed_report(3);
        let length_position = word(&report, offset_word);
        let len = report.len() as u64;

        set_word(&mut report, length_position, len);
        assert_bad_verification(Encoder::parse_signed_report(&report));

        // Would overflow when computing the end of the data
        set_word(&mut report, length_position, u64::MAX);
        assert_bad_verification(Encoder::parse_signed_report(&report));
    }
}

#[test]
fn test_parse_reads_mismatched_signature_arrays() {
    // The parser does not compare rs and ss lengths, verify rejects them with MismatchedSignatures
    let mut report = signed_report(3);
    let ss_length_position = word(&report, SS_OFFSET_WORD);
    set_word(&mut report, ss_length_position, 2);

    let parsed = Encoder::parse_signed_report(&report).unwrap();
    assert_eq!(parsed.rs.len(), 3);
    assert_eq!(parsed.ss.len(), 2);
}

#[test]
fn test_parse_ignores_high_bytes_of_offsets() {
    // Offsets and lengths are read from the last 8 bytes of each word, the rest is ignored. Reports
    // parsed this way are still only accepted if their content is signed by the DON.
    let report = signed_report(3);
    let mut modified = report.clone();
    modified[REPORT_DATA_OFFSET_WORD * WORD_SIZE] = 0xff;

    let parsed = Encoder::parse_signed_report(&report).unwrap();
    let parsed_modified = Encoder::parse_signed_report(&modified).unwrap();
    assert_eq!(parsed.report_data, parsed_modified.report_data);
}
