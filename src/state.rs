use solana_program::{
    program_error::ProgramError,
    program_pack::{Pack, Sealed, IsInitialized}
};


pub struct EscrowAccount {
    pub is_initialized: bool,
    pub bump: u8
    // there is no need to store amount , because we can calculate the transfer amount by subtracting account.lamports - rent_exempt
}

impl EscrowAccount {
    pub fn new(bump: u8) -> Self {
        Self {
            is_initialized: true,
            bump
        }
    }
}

impl IsInitialized for EscrowAccount {
    fn is_initialized(&self) -> bool {
        self.is_initialized
    }
}

impl Sealed for EscrowAccount {}

impl Pack for EscrowAccount {
    const LEN: usize = 2;

    fn pack_into_slice(&self, dst: &mut [u8]) -> () {
        dst.copy_from_slice(&[
            self.is_initialized as u8,
            self.bump
        ]);
    }

    // no need to perform LEN check, because calling Self::unpack() || Self::unpack_unchecked() already does it!
    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        Ok(Self { 
            is_initialized: if src[0] == 1 { true } else { false },
            bump: src[1]
        })
    }
}