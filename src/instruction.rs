use solana_program::program_error::ProgramError;


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
}