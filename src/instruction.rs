use borsh::BorshDeserialize;
use solana_program::program_error::ProgramError;

pub enum CrowdfundingInstruction {
    CreateCampaign { goal: u64, deadline: i64 },
    Contribute { amount: u64 },
    Withdraw,
    Refund,
}

impl CrowdfundingInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&discriminant, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        match discriminant {
            0 => {
                #[derive(BorshDeserialize)]
                struct Args {
                    goal: u64,
                    deadline: i64,
                }
                let args = Args::try_from_slice(rest)
                    .map_err(|_| ProgramError::InvalidInstructionData)?;
                Ok(Self::CreateCampaign { goal: args.goal, deadline: args.deadline })
            }
            1 => {
                #[derive(BorshDeserialize)]
                struct Args {
                    amount: u64,
                }
                let args = Args::try_from_slice(rest)
                    .map_err(|_| ProgramError::InvalidInstructionData)?;
                Ok(Self::Contribute { amount: args.amount })
            }
            2 => Ok(Self::Withdraw),
            3 => Ok(Self::Refund),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
