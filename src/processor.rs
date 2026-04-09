use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

use crate::{
    error::CrowdfundingError,
    instruction::CrowdfundingInstruction,
    state::{Campaign, Contribution},
};

pub struct Processor;

impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = CrowdfundingInstruction::unpack(instruction_data)?;
        match instruction {
            CrowdfundingInstruction::CreateCampaign { goal, deadline } => {
                Self::create_campaign(program_id, accounts, goal, deadline)
            }
            CrowdfundingInstruction::Contribute { amount } => {
                Self::contribute(program_id, accounts, amount)
            }
            CrowdfundingInstruction::Withdraw => Self::withdraw(program_id, accounts),
            CrowdfundingInstruction::Refund => Self::refund(program_id, accounts),
        }
    }

    fn create_campaign(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        goal: u64,
        deadline: i64,
    ) -> ProgramResult {
        let iter = &mut accounts.iter();
        let creator = next_account_info(iter)?;
        let campaign_account = next_account_info(iter)?;
        let vault_pda = next_account_info(iter)?;
        let system_program = next_account_info(iter)?;

        if !creator.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let clock = Clock::get()?;
        if deadline <= clock.unix_timestamp {
            return Err(CrowdfundingError::InvalidDeadline.into());
        }

        let (expected_vault, _bump) =
            Pubkey::find_program_address(&[b"vault", campaign_account.key.as_ref()], program_id);
        if expected_vault != *vault_pda.key {
            return Err(CrowdfundingError::InvalidVault.into());
        }

        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(Campaign::LEN);

        invoke(
            &system_instruction::create_account(
                creator.key,
                campaign_account.key,
                lamports,
                Campaign::LEN as u64,
                program_id,
            ),
            &[creator.clone(), campaign_account.clone(), system_program.clone()],
        )?;

        let campaign = Campaign {
            creator: *creator.key,
            goal,
            raised: 0,
            deadline,
            claimed: false,
        };
        campaign.serialize(&mut &mut campaign_account.data.borrow_mut()[..])?;

        msg!("Campaign created: goal={}, deadline={}", goal, deadline);
        Ok(())
    }

    fn contribute(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        amount: u64,
    ) -> ProgramResult {
        let iter = &mut accounts.iter();
        let contributor = next_account_info(iter)?;
        let campaign_account = next_account_info(iter)?;
        let vault_pda = next_account_info(iter)?;
        let contribution_pda = next_account_info(iter)?;
        let system_program = next_account_info(iter)?;

        if !contributor.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let (expected_vault, _vault_bump) =
            Pubkey::find_program_address(&[b"vault", campaign_account.key.as_ref()], program_id);
        if expected_vault != *vault_pda.key {
            return Err(CrowdfundingError::InvalidVault.into());
        }

        let (expected_contribution, contribution_bump) = Pubkey::find_program_address(
            &[
                b"contribution",
                campaign_account.key.as_ref(),
                contributor.key.as_ref(),
            ],
            program_id,
        );
        if expected_contribution != *contribution_pda.key {
            return Err(ProgramError::InvalidAccountData);
        }

        invoke(
            &system_instruction::transfer(contributor.key, vault_pda.key, amount),
            &[contributor.clone(), vault_pda.clone(), system_program.clone()],
        )?;

        if contribution_pda.data_is_empty() {
            let rent = Rent::get()?;
            let contribution_lamports = rent.minimum_balance(Contribution::LEN);

            invoke_signed(
                &system_instruction::create_account(
                    contributor.key,
                    contribution_pda.key,
                    contribution_lamports,
                    Contribution::LEN as u64,
                    program_id,
                ),
                &[contributor.clone(), contribution_pda.clone(), system_program.clone()],
                &[&[
                    b"contribution",
                    campaign_account.key.as_ref(),
                    contributor.key.as_ref(),
                    &[contribution_bump],
                ]],
            )?;

            let contribution = Contribution { amount };
            contribution.serialize(&mut &mut contribution_pda.data.borrow_mut()[..])?;
        } else {
            let mut contribution =
                Contribution::try_from_slice(&contribution_pda.data.borrow())?;
            contribution.amount = contribution
                .amount
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            contribution.serialize(&mut &mut contribution_pda.data.borrow_mut()[..])?;
        }

        let mut campaign = Campaign::try_from_slice(&campaign_account.data.borrow())?;
        campaign.raised = campaign
            .raised
            .checked_add(amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        campaign.serialize(&mut &mut campaign_account.data.borrow_mut()[..])?;

        msg!("Contributed: {} lamports, total={}", amount, campaign.raised);
        Ok(())
    }

    fn withdraw(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let iter = &mut accounts.iter();
        let creator = next_account_info(iter)?;
        let campaign_account = next_account_info(iter)?;
        let vault_pda = next_account_info(iter)?;
        let system_program = next_account_info(iter)?;

        if !creator.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let mut campaign = Campaign::try_from_slice(&campaign_account.data.borrow())?;

        if *creator.key != campaign.creator {
            return Err(CrowdfundingError::NotCreator.into());
        }

        let clock = Clock::get()?;
        if clock.unix_timestamp < campaign.deadline {
            return Err(CrowdfundingError::DeadlineNotReached.into());
        }

        if campaign.raised < campaign.goal {
            return Err(CrowdfundingError::GoalNotReached.into());
        }

        if campaign.claimed {
            return Err(CrowdfundingError::AlreadyClaimed.into());
        }

        let (expected_vault, vault_bump) =
            Pubkey::find_program_address(&[b"vault", campaign_account.key.as_ref()], program_id);
        if expected_vault != *vault_pda.key {
            return Err(CrowdfundingError::InvalidVault.into());
        }

        let vault_balance = vault_pda.lamports();

        invoke_signed(
            &system_instruction::transfer(vault_pda.key, creator.key, vault_balance),
            &[vault_pda.clone(), creator.clone(), system_program.clone()],
            &[&[b"vault", campaign_account.key.as_ref(), &[vault_bump]]],
        )?;

        campaign.claimed = true;
        campaign.serialize(&mut &mut campaign_account.data.borrow_mut()[..])?;

        msg!("Withdrawn: {} lamports", vault_balance);
        Ok(())
    }

    fn refund(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let iter = &mut accounts.iter();
        let contributor = next_account_info(iter)?;
        let campaign_account = next_account_info(iter)?;
        let vault_pda = next_account_info(iter)?;
        let contribution_pda = next_account_info(iter)?;
        let system_program = next_account_info(iter)?;

        if !contributor.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let campaign = Campaign::try_from_slice(&campaign_account.data.borrow())?;

        let clock = Clock::get()?;
        if clock.unix_timestamp < campaign.deadline {
            return Err(CrowdfundingError::DeadlineNotReached.into());
        }

        if campaign.raised >= campaign.goal {
            return Err(CrowdfundingError::GoalReached.into());
        }

        let (expected_contribution, _contribution_bump) = Pubkey::find_program_address(
            &[
                b"contribution",
                campaign_account.key.as_ref(),
                contributor.key.as_ref(),
            ],
            program_id,
        );
        if expected_contribution != *contribution_pda.key {
            return Err(ProgramError::InvalidAccountData);
        }

        if contribution_pda.data_is_empty() {
            return Err(CrowdfundingError::NoContribution.into());
        }

        let contribution = Contribution::try_from_slice(&contribution_pda.data.borrow())?;
        let refund_amount = contribution.amount;

        if refund_amount == 0 {
            return Err(CrowdfundingError::NoContribution.into());
        }

        let (expected_vault, vault_bump) =
            Pubkey::find_program_address(&[b"vault", campaign_account.key.as_ref()], program_id);
        if expected_vault != *vault_pda.key {
            return Err(CrowdfundingError::InvalidVault.into());
        }

        invoke_signed(
            &system_instruction::transfer(vault_pda.key, contributor.key, refund_amount),
            &[vault_pda.clone(), contributor.clone(), system_program.clone()],
            &[&[b"vault", campaign_account.key.as_ref(), &[vault_bump]]],
        )?;

        let contribution_lamports = contribution_pda.lamports();
        **contribution_pda.lamports.borrow_mut() = 0;
        **contributor.lamports.borrow_mut() = contributor
            .lamports()
            .checked_add(contribution_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        contribution_pda.data.borrow_mut().fill(0);

        msg!("Refunded: {} lamports", refund_amount);
        Ok(())
    }
}
