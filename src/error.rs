use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CrowdfundingError {
    #[error("Deadline must be in the future")]
    InvalidDeadline,
    #[error("Deadline has not been reached yet")]
    DeadlineNotReached,
    #[error("Funding goal has not been reached")]
    GoalNotReached,
    #[error("Funding goal was reached; refund not allowed")]
    GoalReached,
    #[error("Funds have already been claimed")]
    AlreadyClaimed,
    #[error("Caller is not the campaign creator")]
    NotCreator,
    #[error("Vault PDA does not match expected address")]
    InvalidVault,
    #[error("No contribution record found for this contributor")]
    NoContribution,
    #[error("Campaign deadline has passed; contributions no longer accepted")]
    CampaignEnded,
}

impl From<CrowdfundingError> for ProgramError {
    fn from(e: CrowdfundingError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
