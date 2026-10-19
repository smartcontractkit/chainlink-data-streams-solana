use crate::domain::{Report, SignedReport};
use crate::errors::ErrorCode;
use ethabi::{decode, ParamType, Token};
use std::convert::TryInto;

const WORD_SIZE: usize = 32; // EVM word size in bytes

pub struct Encoder {}

impl Encoder {
    pub fn decode(
        param_types: &[ParamType],
        data: &[u8],
    ) -> Result<Vec<Token>, ethabi::Error> {
        decode(param_types, data)
    }

    // Parses EVM encoded report without copying data
    pub fn parse_signed_report<'a>(
        signed_report: &'a [u8],
    ) -> Result<SignedReport<'a>, ErrorCode> {

        const STATIC_SIZE: usize = 7 * WORD_SIZE; // Total size of static data

        // Ensure the input has at least the minimum required length
        if signed_report.len() < STATIC_SIZE {
            return Err(ErrorCode::BadVerification);
        }

        // Parse report_context (bytes32[3]) as slices
        let report_context_slice = &signed_report[0..3 * WORD_SIZE];
        let report_context: &'a [[u8; 32]; 3] = unsafe {
            // Cast the slice to a reference to [[u8; 32]; 3]
            &*(report_context_slice.as_ptr() as *const [[u8; 32]; 3])
        };

        // Parse offsets for dynamic data
        let report_data_offset =
            Self::read_u256_as_usize(&signed_report[3 * WORD_SIZE..4 * WORD_SIZE])?;
        let rs_offset =
            Self::read_u256_as_usize(&signed_report[4 * WORD_SIZE..5 * WORD_SIZE])?;
        let ss_offset =
            Self::read_u256_as_usize(&signed_report[5 * WORD_SIZE..6 * WORD_SIZE])?;

        // Parse raw_vs (bytes32)
        let raw_vs: &'a [u8; 32] = signed_report[6 * WORD_SIZE..7 * WORD_SIZE]
            .try_into()
            .map_err(|_| ErrorCode::BadVerification)?;

        // Parse report_data (bytes) as a slice
        let report_data = Self::read_bytes(signed_report, report_data_offset)?;

        // Parse rs (bytes32[]) as a slice
        let rs = Self::read_bytes32_array(signed_report, rs_offset)?;

        // Parse ss (bytes32[]) as a slice
        let ss = Self::read_bytes32_array(signed_report, ss_offset)?;

        Ok(SignedReport {
            report_context,
            report_data,
            rs,
            ss,
            raw_vs,
        })
    }

    fn read_u256_as_usize(slice: &[u8]) -> Result<usize, ErrorCode> {
        // Ensure the input has at least the minimum required length
        if slice.len() != WORD_SIZE{
            return Err(ErrorCode::BadVerification);
        }

        // Read the last 8 bytes as a big-endian u64
        let offset = u64::from_be_bytes(
            slice[24..WORD_SIZE].try_into().map_err(|_| ErrorCode::BadVerification)?,
        ) as usize;
       
        Ok(offset)
    }

    fn read_bytes(
        data: &[u8],
        offset: usize,
    ) -> Result<&[u8], ErrorCode> {
        // Ensure the input has at least the minimum required length, without overflowing on malformed offsets
        if offset.checked_add(WORD_SIZE).map_or(true, |end| end > data.len()) {
            return Err(ErrorCode::BadVerification);
        }

        // Read the length of the bytes array
        let len = Self::read_u256_as_usize(&data[offset..offset + WORD_SIZE])?;

        // Start of the array is after the length
        let start = offset + WORD_SIZE;

        // Check for malformed length which could cause overflow
        let end = start
            .checked_add(len)
            .ok_or(ErrorCode::BadVerification)?;
        if end > data.len() {
            return Err(ErrorCode::BadVerification);
        }

        Ok(&data[start..end])
    }

    fn read_bytes32_array(
        data: &[u8],
        offset: usize,
    ) -> Result<&[[u8; WORD_SIZE]], ErrorCode> {
        // Ensure the input has at least the minimum required length, without overflowing on malformed offsets
        if offset.checked_add(WORD_SIZE).map_or(true, |end| end > data.len()) {
            return Err(ErrorCode::BadVerification);
        }

        // Read the length of the bytes array
        let len = Self::read_u256_as_usize(&data[offset..offset + WORD_SIZE])?;

        // Start of the array is after the length
        let start = offset + WORD_SIZE;

        // Calc size and check it's not malformed
        let total_size = len
            .checked_mul(WORD_SIZE)
            .ok_or(ErrorCode::BadVerification)?;
        
        // Calc end and check it's not malformed
        let end = start
            .checked_add(total_size)
            .ok_or(ErrorCode::BadVerification)?;

        // Check the calculated end is not malformed
        if end > data.len() {
            return Err(ErrorCode::BadVerification);
        }

        // Get a reference to the bytes array and check it contains a multiple of 32 bytes
        let bytes_slice = &data[start..end];
        if bytes_slice.len() % WORD_SIZE != 0 {
            return Err(ErrorCode::BadVerification);
        }

        // Get a pointer to the bytes array and calculate the length
        let array_ptr = bytes_slice.as_ptr() as *const [u8; WORD_SIZE];
        let array_len = bytes_slice.len() / WORD_SIZE;
        let array_slice = unsafe { std::slice::from_raw_parts(array_ptr, array_len) };

        Ok(array_slice)
    }

    pub fn parse_report_details_from_report<'a>(
        report_data: &'a [u8],
    ) -> Result<Report<'a>, ErrorCode> {
    
        // Extract feed_id ref from report_data
        let feed_id: &'a [u8; WORD_SIZE] = report_data
            .get(0..WORD_SIZE)
            .ok_or(ErrorCode::BadVerification)?
            .try_into()
            .map_err(|_| ErrorCode::BadVerification)?;

        // Extract report_timestamp from report_data
        let report_timestamp = u32::from_be_bytes(
            report_data
                .get(92..96)
                .ok_or(ErrorCode::BadVerification)?
                .try_into()
                .map_err(|_| ErrorCode::BadVerification)?,
        );

        Ok(Report {
            feed_id,
            report_timestamp,
        })
    }
}