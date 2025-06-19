pub mod state;
pub mod processor;
pub mod entrypoint;
pub mod instruction;

use solana_program::{
    declare_id,
    pubkey::Pubkey,
    program_error::ProgramError
};

declare_id!("E6v3tbZyZAthzd5JCPJgd3TmLXL3VirKxib9XHjyKTjL");

pub fn get_escrow_seeds<'a>(payer_pkey: &'a Pubkey, recipient_pkey: &'a Pubkey) -> (&'a [u8], &'a [u8], &'a [u8]) {
    (
        b"escrow",
        payer_pkey.as_ref(),
        recipient_pkey.as_ref()
    )
}

pub fn check_provided_pda(
    payer_pkey: &Pubkey, 
    recipient_pkey: &Pubkey,
    escrow_pda: &Pubkey,
    bump: u8
) -> Result<(), ProgramError> {
    let (seed1, seed2, seed3) = get_escrow_seeds(payer_pkey, recipient_pkey);
    let expected_pda: Pubkey = Pubkey::create_program_address(
        &[seed1, seed2, seed3, &[bump]], 
        &crate::ID
    )?;

    if escrow_pda != &expected_pda {
        return Err(ProgramError::InvalidInstructionData);
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use solana_program_test::{BanksClient, ProgramTest, processor};
    use solana_sdk::{
        hash::Hash,
        system_program,
        pubkey::Pubkey,
        signer::{
            Signer,
            keypair::Keypair
        },
        message::Message,
        transaction::Transaction,
        instruction::{Instruction, AccountMeta}
    };

    #[tokio::test]
    async fn test_init_escrow_instruction() -> Result<(), Box<dyn std::error::Error>> {
        let program_test: ProgramTest = ProgramTest::new(
            "escrow",
            crate::ID,
            processor!(super::entrypoint::process_instruction)
        );
    
        // 1.1 init Client, payer, get latest blockhash
        let (banks_client, payer, latest_blockhash) = program_test.start().await;
        let payer_pkey: Pubkey = payer.pubkey();

        // 1.2 init recipient & derive escrow PDA 
        let recipient: Keypair = Keypair::new(); 
        let recipient_pkey: Pubkey = recipient.pubkey();

        let (escrow_pda, _bump) = derive_escrow_pda(&payer_pkey, &recipient_pkey);

        // 2. init escrow
        init_escrow(&banks_client, &payer, &payer_pkey, &recipient_pkey, &escrow_pda, latest_blockhash).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_init_and_complete_escrow_instructions() -> Result<(), Box<dyn std::error::Error>> {
        let program_test: ProgramTest = ProgramTest::new(
            "escrow",
            crate::ID,
            processor!(super::entrypoint::process_instruction)
        );
    
        // 1.1 init Client, payer, get latest blockhash
        let (banks_client, payer, latest_blockhash) = program_test.start().await;
        let payer_pkey: Pubkey = payer.pubkey();

        // 1.2 init recipient & derive escrow PDA 
        let recipient: Keypair = Keypair::new(); 
        let recipient_pkey: Pubkey = recipient.pubkey();

        let (escrow_pda, _bump) = derive_escrow_pda(&payer_pkey, &recipient_pkey);

        // 2. init escrow
        init_escrow(&banks_client, &payer, &payer_pkey, &recipient_pkey, &escrow_pda, latest_blockhash).await?;

        // 3. complete escrow
        // 3.1 craft ix & tx
        let complete_escrow_ix: Instruction = Instruction::new_with_bytes(
            crate::ID, 
            &[1], 
            vec![
                AccountMeta::new(payer_pkey, true),
                AccountMeta::new(recipient_pkey, false),
                AccountMeta::new(escrow_pda, false)
            ]
        );
        let message: Message = Message::new(&[complete_escrow_ix], Some(&payer_pkey));
        let mut complete_escrow_tx: Transaction = Transaction::new_unsigned(message);

        // 3.2 sign complete escrow tx & send it
        complete_escrow_tx.sign(&[&payer], latest_blockhash);
        banks_client.process_transaction(complete_escrow_tx).await?;

        Ok(())
    }

    #[tokio::test]
    async fn init_and_close_escrow_instructions() -> Result<(), Box<dyn std::error::Error>> {
        let program_test: ProgramTest = ProgramTest::new(
            "escrow",
            crate::ID,
            processor!(super::entrypoint::process_instruction)
        );
    
        // 1.1 init Client, payer, get latest blockhash
        let (banks_client, payer, latest_blockhash) = program_test.start().await;
        let payer_pkey: Pubkey = payer.pubkey();

        // 1.2 init recipient & derive escrow PDA 
        let recipient: Keypair = Keypair::new(); 
        let recipient_pkey: Pubkey = recipient.pubkey();

        let (escrow_pda, _bump) = derive_escrow_pda(&payer_pkey, &recipient_pkey);

        // 2. init escrow
        init_escrow(&banks_client, &payer, &payer_pkey, &recipient_pkey, &escrow_pda, latest_blockhash).await?;

        // 3. close escrow
        let close_escrow_ix: Instruction = Instruction::new_with_bytes(
            crate::ID, 
            &[2], 
            vec![
                AccountMeta::new(payer_pkey, true),
                AccountMeta::new_readonly(recipient_pkey, false),
                AccountMeta::new(escrow_pda, false)
            ]
        );
        let message: Message = Message::new(&[close_escrow_ix], Some(&payer_pkey));
        let mut close_escrow_tx: Transaction = Transaction::new_unsigned(message);

        // 3.2 sign complete escrow tx & send it
        close_escrow_tx.sign(&[&payer], latest_blockhash);
        banks_client.process_transaction(close_escrow_tx).await?;

        Ok(())
    }

    async fn init_escrow(
        banks_client: &BanksClient,
        payer: &Keypair,
        payer_pkey: &Pubkey,
        recipient_pkey: &Pubkey,
        escrow_pda: &Pubkey,
        latest_blockhash: Hash
    ) -> Result<(), Box<dyn std::error::Error>> {        
        // craft init ix & init tx
        let mut init_ix_payload: Vec<u8> = Vec::with_capacity(9);
        init_ix_payload.push(0);
        init_ix_payload.extend_from_slice(&u64::to_le_bytes(101101101));

        let initialize_escrow_ix: Instruction = Instruction::new_with_bytes(
            crate::ID, 
            &init_ix_payload, 
            vec![
                AccountMeta::new(*payer_pkey, true),
                AccountMeta::new_readonly(*recipient_pkey, false),
                AccountMeta::new(*escrow_pda, false),
                AccountMeta::new_readonly(system_program::ID, false)
            ]
        );
        let message: Message = Message::new(&[initialize_escrow_ix], Some(&payer_pkey));
        let mut initialize_escrow_tx: Transaction = Transaction::new_unsigned(message);

        // sign init escrow tx & send it
        initialize_escrow_tx.sign(&[&payer], latest_blockhash);
        banks_client.process_transaction(initialize_escrow_tx).await?;

        Ok(())
    }
    
    fn derive_escrow_pda(payer_pkey: &Pubkey, recipient_pkey: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"escrow",
                payer_pkey.as_ref(),
                recipient_pkey.as_ref()
            ], 
            &crate::ID
        )
    }
}