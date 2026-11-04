use arrayref::array_ref;
use ethabi::Token;
use secp256k1::{rand, Message, PublicKey, Secp256k1, SecretKey};
use tiny_keccak::{Hasher, Keccak};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone)]
pub struct Signer {
    pub private_key: [u8; 32],
    pub signer_address: [u8; 20],
}

pub trait Report {
    fn encode(&self) -> Vec<u8>;
    fn dummy(observation_timestamp: Option<u32>) -> Self;
}

#[derive(Debug, Clone)]
pub struct DummyReport {
    feed_id: [u8; 32],           // bytes32
    lower_timestamp: u32,        // uint32
    observations_timestamp: u32, // uint32
    benchmark_price: String,     // int192
}

impl Report for DummyReport {
    fn encode(&self) -> Vec<u8> {
        let tokens = vec![
            Token::FixedBytes(self.feed_id.to_vec()),
            Token::Uint(self.lower_timestamp.into()),
            Token::Uint(self.observations_timestamp.into()),
            Token::Int(ethabi::ethereum_types::U256::from_dec_str(&self.benchmark_price).unwrap()),
        ];

        ethabi::encode(&tokens)
    }

    fn dummy(observation_timestamp: Option<u32>) -> Self {
        let current_ts: u32 = observation_timestamp.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Error: Timestamp in the past")
                .as_secs() as u32
        });

        DummyReport {
            feed_id: [0u8; 32],
            lower_timestamp: 1_727_467_477,
            observations_timestamp: current_ts,
            benchmark_price: "655442225238888900".to_string(),
        }
    }
}

// representing int192 as string for simplicity
#[derive(Debug, Clone)]
pub struct V3Report {
    feed_id: [u8; 32],           // bytes32
    lower_timestamp: u32,        // uint32
    observations_timestamp: u32, // uint32
    native_fee: String,          // uint192
    link_fee: String,            // uint192
    valid_from_timestamp: u32,   // uint32
    benchmark_price: String,     // int192
    bid: String,                 // int192
    ask: String,                 // int192
}

impl Report for V3Report {
    fn encode(&self) -> Vec<u8> {
        let tokens = vec![
            Token::FixedBytes(self.feed_id.to_vec()),
            Token::Uint(self.lower_timestamp.into()),
            Token::Uint(self.observations_timestamp.into()),
            Token::Uint(ethabi::ethereum_types::U256::from_dec_str(&self.native_fee).unwrap()),
            Token::Uint(ethabi::ethereum_types::U256::from_dec_str(&self.link_fee).unwrap()),
            Token::Uint(self.valid_from_timestamp.into()),
            Token::Int(ethabi::ethereum_types::U256::from_dec_str(&self.benchmark_price).unwrap()),
            Token::Int(ethabi::ethereum_types::U256::from_dec_str(&self.bid).unwrap()),
            Token::Int(ethabi::ethereum_types::U256::from_dec_str(&self.ask).unwrap()),
        ];

        ethabi::encode(&tokens)
    }

    fn dummy(observation_timestamp: Option<u32>) -> Self {
        let current_ts: u32 = observation_timestamp.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Error: Timestamp in the past")
                .as_secs() as u32
        });

        const FEED_ID_V3: [u8; 32] = [
            0x00, 0x03, 0x0a, 0xb7, 0xd0, 0x2f, 0xbb, 0xa9, 0xc6, 0x30, 0x4f, 0x98, 0x82, 0x45, 0x24,
            0x40, 0x7b, 0x1f, 0x49, 0x47, 0x41, 0x17, 0x43, 0x20, 0xcf, 0xd1, 0x7a, 0x2c, 0x22, 0xee,
            0xc1, 0xde,
        ];

        V3Report {
            feed_id: FEED_ID_V3,
            lower_timestamp: 1_727_467_477,
            observations_timestamp: current_ts,
            native_fee: "118647852657900".to_string(),
            link_fee: "25234531311164200".to_string(),
            valid_from_timestamp: 0,
            benchmark_price: "655442225238888900".to_string(),
            bid: "655292586749804350".to_string(),
            ask: "655615783467747900".to_string(),
        }
    }
}

/// Generates signatures from signers for the given report and report context.
fn generate_signer_signatures(
    report: &[u8],
    report_context: [[u8; 32]; 3],
    signers: &[Signer],
) -> (Vec<[u8; 32]>, Vec<[u8; 32]>, Vec<u8>) {
    let mut rs = Vec::with_capacity(signers.len());
    let mut ss = Vec::with_capacity(signers.len());
    let mut vs = Vec::with_capacity(signers.len());

    // Compute hash = keccak256(keccak256(report) + reportContext)
    let hash_report = keccak256(report);

    // Prepare the data to hash: keccak256(report) concatenated with reportContext
    let mut data_to_hash = Vec::new();
    data_to_hash.extend_from_slice(&hash_report);
    for context_part in &report_context {
        // no padding required as the context is already 32 bytes
        data_to_hash.extend_from_slice(context_part);
    }

    // Compute the final hash
    let hash = keccak256(&data_to_hash);

    // Create a Secp256k1 context for signing
    let secp = Secp256k1::new();

    // Sign the hash with each signer's private key
    for signer in signers {
        // Create a SecretKey from the mock private key
        let secret_key =
            SecretKey::from_byte_array(&signer.private_key).expect("Private key must be 32 bytes");

        let message: Message = Message::from_digest(hash);

        let sig = secp.sign_ecdsa_recoverable(&message, &secret_key);

        let (recovery_id, serialized_sig) = sig.serialize_compact();

        let r = *array_ref![serialized_sig, 0, 32];
        let s = *array_ref![serialized_sig, 32, 32];

        rs.push(r);
        ss.push(s);

        // Get the recovery ID as u8 (0 or 1)
        let recovery_id_i32: i32 = recovery_id.into();
        let recovery_id_u8: u8 = recovery_id_i32 as u8;

        vs.push(recovery_id_u8);
    }

    (rs, ss, vs)
}

fn keccak256(input: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak::v256();
    hasher.update(input);
    let mut output = [0u8; 32];
    hasher.finalize(&mut output);
    output
}

/// Generates the encoded blob
fn generate_encoded_blob(
    report: &impl Report,
    report_context: [[u8; 32]; 3],
    signers: &[Signer],
) -> Vec<u8> {
    let report_bytes = report.encode();

    let (rs, ss, vs) = generate_signer_signatures(&report_bytes, report_context, signers);

    // Create tokens in the required format
    let tokens = vec![
        // context[3]
        Token::FixedArray(
            report_context.iter()
                .map(|rc| Token::FixedBytes(rc.to_vec()))
                .collect()
        ),
        // report bytes
        Token::Bytes(report_bytes),
        // rs[]
        Token::Array(
            rs.iter()
                .map(|r| Token::FixedBytes(r.to_vec()))
                .collect()
        ),
        // ss[]
        Token::Array(
            ss.iter()
                .map(|s| Token::FixedBytes(s.to_vec()))
                .collect()
        ),
        // v as bytes32
        Token::FixedBytes(vs.to_vec()),
    ];

    ethabi::encode(&tokens)
}

pub fn get_signers(count: usize) -> Vec<Signer> {
    // Implement your logic to retrieve or generate signers
    (0..count).map(|_| create_random_signer()).collect()
}

fn create_random_signer() -> Signer {
    let secp = Secp256k1::new();

    let secret_key = SecretKey::new(&mut rand::thread_rng());
    let public_key = PublicKey::from_secret_key(&secp, &secret_key);

    let serialized_pubkey = public_key.serialize_uncompressed();
    // Exclude the first byte (0x04)
    // The first byte is a prefix (0x04) indicating that the key is uncompressed.
    // The next 32 bytes are the X coordinate.
    // The last 32 bytes are the Y coordinate.
    let pubkey_bytes = &serialized_pubkey[1..];
    // Compute Keccak-256 hash
    let pubkey_hash: [u8; 32] = keccak256(pubkey_bytes);
    // Take the last 20 bytes as the address
    let signer_address: [u8; 20] = pubkey_hash[12..].try_into().unwrap();

    Signer {
        private_key: secret_key.secret_bytes(),
        signer_address,
    }
}

/// Generates a config digest with the LLO prefix (0x0009) for tests. Different `n` give different digests
pub fn test_config_digest(n: u8) -> [u8; 32] {
    let mut digest = keccak256(&[n]);
    digest[0] = 0x00;
    digest[1] = 0x09;
    digest
}

pub fn generate_report_with_signers<T: Report>(total_signers: usize, signers_per_report: usize, observation_timestamp: Option<u32>, override_signers: Option<Vec<Signer>>) -> (Vec<u8>, Vec<[u8; 20]>) {
    generate_report_with_signers_and_digest::<T>([0u8; 32], total_signers, signers_per_report, observation_timestamp, override_signers)
}

/// Same as `generate_report_with_signers` but signs the report for the given config digest (report_context[0])
pub fn generate_report_with_signers_and_digest<T: Report>(config_digest: [u8; 32], total_signers: usize, signers_per_report: usize, observation_timestamp: Option<u32>, override_signers: Option<Vec<Signer>>) -> (Vec<u8>, Vec<[u8; 20]>) {
    let report_context: [[u8; 32]; 3] = [config_digest, [0u8; 32], [0u8; 32]];
    let signer_set = match override_signers {
        Some(signers) => signers,
        None => get_signers(total_signers)
    };

    assert!(signers_per_report <= signer_set.len(), "signers_per_report must be <= total_signers");

    let signers = &signer_set[..signers_per_report];

    (generate_encoded_blob(&T::dummy(observation_timestamp), report_context, signers), signer_set.iter().map(|s| s.signer_address).collect())
}

#[cfg(test)]
mod tests {
    use crate::report::{generate_encoded_blob, V3Report, Report, get_signers};
    use hex::encode as hex_encode;

    #[test]
    fn test_encode_v3_report() {
        let report_context = [[0u8; 32], [0u8; 32], [0u8; 32]];

        let signers = get_signers(4);

        let report = V3Report::dummy(None);
        let signed_report = generate_encoded_blob(&report, report_context, &signers);

        println!("Signed report: {}", hex_encode(&signed_report));
    }

}
