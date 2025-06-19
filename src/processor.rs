use solana_program::{
    rent::Rent,
    sysvar::Sysvar,
    pubkey::Pubkey,
    system_instruction,
    instruction::Instruction,
    entrypoint::ProgramResult,
    program::invoke_signed,
    program_pack::Pack,
    program_error::ProgramError,
    account_info::{AccountInfo, next_account_info},
};
use super::{
    state::EscrowAccount,
    instruction::EscrowInstruction
};


pub struct Processor;

impl Processor {
    pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
        let instruction: EscrowInstruction = EscrowInstruction::unpack(data)?;

        match instruction {
            EscrowInstruction::Initialize { amount } => Self::process_initialize_escrow(program_id, accounts, amount)?,
            EscrowInstruction::Complete => Self::process_complete_escrow(program_id, accounts)?,
            EscrowInstruction::Close => todo!()      // todo add close
        };

        Ok(())
    }

    fn process_initialize_escrow(program_id: &Pubkey, accounts: &[AccountInfo], amount: u64) -> ProgramResult {
        let rent_exemp: u64 = Rent::get()?.minimum_balance(EscrowAccount::LEN);
        
        if amount < rent_exemp {
            return Err(ProgramError::InsufficientFunds);
        }
        
        let accounts_iter = &mut accounts.iter();
        
        let payer_account: &AccountInfo = next_account_info(accounts_iter)?;
        let recipient_account: &AccountInfo = next_account_info(accounts_iter)?;
        let escrow_account: &AccountInfo = next_account_info(accounts_iter)?;  // pda
        let system_program_account: &AccountInfo = next_account_info(accounts_iter)?;
        
        let (seed1, seed2, seed3) = (
            b"escrow",
            payer_account.key.as_ref(),
            recipient_account.key.as_ref()
        );
        let (expected_pda, bump) = Pubkey::find_program_address(
            &[seed1, seed2, seed3],
            program_id
        );

        if &expected_pda != escrow_account.key {
            return Err(ProgramError::InvalidInstructionData);
        }

        let signers_seeds: &[&[u8]] = &[seed1, seed2, seed3, &[bump]];
        let total_amount: u64 = rent_exemp + amount;

        // 1. create pda account
        let create_ix: Instruction = system_instruction::create_account(
            payer_account.key, 
            escrow_account.key, 
            total_amount, 
            EscrowAccount::LEN as u64, 
            program_id
        );
        invoke_signed(
            &create_ix, 
            &[
                payer_account.clone(),
                escrow_account.clone(),
                system_program_account.clone()
            ],
            &[signers_seeds]
        )?;

        // 2. init pda account
        let escrow_instance: EscrowAccount = EscrowAccount::new(bump);
        let escrow_data: &mut [u8] = &mut **escrow_account.data.borrow_mut();
        escrow_instance.pack_into_slice(escrow_data);

        Ok(())
    }

    fn process_complete_escrow(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let payer_account: &AccountInfo = next_account_info(accounts_iter)?;
        let recipient_account: &AccountInfo = next_account_info(accounts_iter)?;
        let escrow_account: &AccountInfo = next_account_info(accounts_iter)?;

        if escrow_account.owner != program_id {
            return Err(ProgramError::IncorrectProgramId);
        }

        // 1. unpack EscrowAccount (check if it's initialized & extract bump)
        let escrow_data: &[u8] = &**escrow_account.data.borrow(); 
        let escrow_instance: EscrowAccount = EscrowAccount::unpack(escrow_data)?;
        
        // 2. create `expected_pda` and check the match with provided pda
        let expected_pda: Pubkey = Pubkey::create_program_address(
            &[
                b"escrow",
                payer_account.key.as_ref(),
                recipient_account.key.as_ref(),
                &[escrow_instance.bump]
            ], 
            &crate::ID
        )?;

        if escrow_account.key != &expected_pda {
            return Err(ProgramError::InvalidInstructionData);
        }

        let rent_exemp: u64 = Rent::get()?.minimum_balance(EscrowAccount::LEN);
        let vault_balance: u64 = escrow_account.lamports() - rent_exemp;

        **recipient_account.lamports.borrow_mut() = recipient_account.lamports()
            .checked_add(vault_balance)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        **escrow_account.lamports.borrow_mut() -= vault_balance;

        Ok(())
    }
}