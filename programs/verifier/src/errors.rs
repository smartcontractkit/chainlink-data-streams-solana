use anchor_lang::error_code;

#[error_code]
pub enum ErrorCode {
    #[msg("Zero Address")]
    ZeroAddress,
    #[msg("Fault tolerance must be a positive non-zero value")]
    FaultToleranceMustBePositive,
    #[msg("Too many signers provided")]
    ExcessSigners,
    #[msg("Insufficient number of signers provided")]
    InsufficientSigners,
    #[msg("Non-unique signatures provided")]
    NonUniqueSignatures,
    #[msg("Activation time cannot be in the future")]
    BadActivationTime,
    #[msg("DonConfig already exists")]
    DonConfigAlreadyExists,
    #[msg("Bad verification")]
    BadVerification,
    #[msg("Mismatched signatures")]
    MismatchedSignatures,
    #[msg("No Signers")]
    NoSigners,
    #[msg("DonConfig does not exist")]
    DonConfigDoesNotExist,
    #[msg("Invalid PDA")]
    InvalidPDA,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Invalid Access Controller")]
    InvalidAccessController,
    #[msg("Invalid Config Account")]
    InvalidConfigAccount,
    #[msg("Max number of configs reached")]
    MaxNumberOfConfigsReached,
    #[msg("Config is deactivated")]
    ConfigDeactivated,
    #[msg("Invalid inputs")]
    InvalidInputs,
    // New errors must be appended to keep existing error codes stable for integrators
    #[msg("Config digest is empty")]
    DigestEmpty,
    #[msg("Config digest is not set")]
    DigestNotSet,
    #[msg("Config digest is already set")]
    ConfigDigestAlreadySet,
}
