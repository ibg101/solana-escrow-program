use solana_program::{
    system_program,
    entrypoint::ProgramResult,
    program_error::ProgramError,
    account_info::AccountInfo,
};


pub enum EscrowInstruction {
    Initialize { amount: u64 },
    Complete,
    Close
}

impl EscrowInstruction {
    pub fn unpack(data: &[u8]) -> Result<Self, ProgramError> {
        let (instr_type, rest) = data.split_at(1);
        
        Ok(match instr_type[0] {
            0 => {
                let amount: u64 = u64::from_le_bytes(
                    rest.try_into().map_err(|_| ProgramError::InvalidInstructionData)?
                );
                Self::Initialize { amount }
            },
            1 => EscrowInstruction::Complete,
            2 => EscrowInstruction::Close,
            _ => return Err(ProgramError::InvalidInstructionData)
        })
    }

    /// This method does the following:
    /// 
    /// * Sets `escrow_account.lamports` to 0, transfering them to the `payer`.
    /// * Assigns ownership of `escrow_account` to the `SystemProgram`.
    /// * Reallocates space in `escrow_account`, zeroing the data.
    pub fn close_account(
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