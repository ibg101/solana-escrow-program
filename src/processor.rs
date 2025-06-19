use solana_program::{
    rent::Rent,
    sysvar::Sysvar,
    pubkey::Pubkey,
    system_program,
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
            EscrowInstruction::Close => Self::process_close_escrow(program_id, accounts)?
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
        
        let (seed1, seed2, seed3) = crate::get_escrow_seeds(payer_account.key, recipient_account.key);
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
        let escrow_data = escrow_account.data.borrow(); 
        let escrow_instance: EscrowAccount = EscrowAccount::unpack(&**escrow_data)?;
        std::mem::drop(escrow_data);  // explicitly dropping ref, because we call escrow_account.data.borrow_mut() in close_account()

        // 2. create `expected_pda` and check the match with provided pda
        crate::check_provided_pda(
            payer_account.key,
            recipient_account.key,
            escrow_account.key,
            escrow_instance.bump
        )?;

        // 3. transfer locked lamports in the contract to the recipient & close `EscrowAccount`.
        // Note, that we MUST NOT subtract the balance of `EscrowAccount`, because `EscrowInstruction::close()` already handles it.
        let rent_exemp: u64 = Rent::get()?.minimum_balance(EscrowAccount::LEN);
        let locked_amount: u64 = escrow_account.lamports() - rent_exemp;

        **recipient_account.lamports.borrow_mut() = recipient_account.lamports()
            .checked_add(locked_amount)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        Self::_process_close_escrow(payer_account, escrow_account, rent_exemp)?;

        Ok(())
    }

    fn process_close_escrow(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
        let accounts_iter = &mut accounts.iter();

        let payer_account: &AccountInfo = next_account_info(accounts_iter)?;
        // this field is only used for pda check, namely `recipient_account.key`. 
        // (we could teoretically store `recipient_pkey` in the `escrow_account.data`, but since it's used only here, it's an overkill)
        let recipient_account: &AccountInfo = next_account_info(accounts_iter)?;
        let escrow_account: &AccountInfo = next_account_info(accounts_iter)?;

        if escrow_account.owner != program_id {
            return Err(ProgramError::IncorrectProgramId);
        }

        // 1. unpack EscrowAccount (check if it's initialized & extract bump)
        let escrow_data = escrow_account.data.borrow();
        let escrow_instance: EscrowAccount = EscrowAccount::unpack(&**escrow_data)?;
        std::mem::drop(escrow_data);  // explicitly dropping ref, because we call escrow_account.data.borrow_mut() in close_account()
        
        // 2. create `expected_pda` and check the match with provided pda
        crate::check_provided_pda(
            payer_account.key,
            recipient_account.key,
            escrow_account.key,
            escrow_instance.bump
        )?;

        // 3. close `EscrowAccount`
        let total_amount: u64 = escrow_account.lamports();

        Self::_process_close_escrow(payer_account, escrow_account, total_amount)
    }

    /// This method does the following:
    /// 
    /// * Sets `escrow_account.lamports` to 0, transfering them to the `payer`.
    /// * Assigns ownership of `escrow_account` to the `SystemProgram`.
    /// * Reallocates space in `escrow_account`, zeroing the data.
    fn _process_close_escrow(
        payer_account: &AccountInfo,
        escrow_account: &AccountInfo,
        lamports: u64
    ) -> ProgramResult {
        **payer_account.lamports.borrow_mut() = payer_account.lamports()
            .checked_add(lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        **escrow_account.lamports.borrow_mut() = 0;

        escrow_account.assign(&system_program::ID);
        
        escrow_account.realloc(0, true)?;

        Ok(())
    }
}