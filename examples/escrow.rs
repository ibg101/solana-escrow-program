use solana_sdk::{
    hash::Hash, 
    instruction::{AccountMeta, Instruction}, 
    message::Message, 
    native_token::LAMPORTS_PER_SOL, 
    pubkey::Pubkey, 
    signature::{Keypair, Signature}, 
    signer::Signer, 
    system_program,
    transaction::Transaction
};
use solana_client::nonblocking::rpc_client::RpcClient;


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv()?;
    env_logger::init();

    log::info!("Running example!");

    let url: String = String::from("http://127.0.0.1:8899");
    let rpc_client: RpcClient = RpcClient::new(url);

    // 1. init payer & recipient
    let Accounts { payer, recipient } = init_payer_and_recipient(&rpc_client).await?;

    // 2. derive escrow pda
    let (escrow_pda, _bump) = Pubkey::find_program_address(
        &[
            b"escrow",
            payer.pkey.as_ref(),
            recipient.pkey.as_ref()
        ], 
        &escrow::ID
    );
        
    // 3. craft init ix & init tx
    let mut init_ix_payload: Vec<u8> = Vec::with_capacity(9);
    init_ix_payload.push(0);
    init_ix_payload.extend_from_slice(&u64::to_le_bytes(101101101));

    let initialize_escrow_ix: Instruction = Instruction::new_with_bytes(
        escrow::ID, 
        &init_ix_payload, 
        vec![
            AccountMeta::new(payer.pkey, true),
            AccountMeta::new_readonly(recipient.pkey, false),
            AccountMeta::new(escrow_pda, false),
            AccountMeta::new_readonly(system_program::ID, false)
        ]
    );
    let message: Message = Message::new(&[initialize_escrow_ix], Some(&payer.pkey));
    let mut initialize_escrow_tx: Transaction = Transaction::new_unsigned(message);

    // 4. get latest_blockhash
    let latest_blockhash: Hash = rpc_client.get_latest_blockhash().await?;

    // 5. sign init escrow tx & send it
    initialize_escrow_tx.sign(&[&payer.keypair], latest_blockhash);
    send_tx_and_print_result(&rpc_client, &initialize_escrow_tx).await?;

    // // 6. craft complete ix & complete tx
    // let complete_escrow_ix: Instruction = Instruction::new_with_bytes(
    //     escrow::ID, 
    //     &[1], 
    //     vec![
    //         AccountMeta::new(payer.pkey, true),
    //         AccountMeta::new(recipient.pkey, false),
    //         AccountMeta::new(escrow_pda, false)
    //     ]
    // );
    // let message: Message = Message::new(&[complete_escrow_ix], Some(&payer.pkey));
    // let mut complete_escrow_tx: Transaction = Transaction::new_unsigned(message);

    // // 7. sign complete tx & send it
    // complete_escrow_tx.sign(&[&payer.keypair], latest_blockhash);
    // send_tx_and_print_result(&rpc_client, &complete_escrow_tx).await?;

    // this is an alternative way: 
    // (either complete escrow, or close it. note, that complete escrow also closes EscrowAccount at the end,
    // so we can freely ignore testing this instruction, since they both use the same `EscrowInstruction::close_account()` method)
    // 6. craft close ix & close ix
    let close_escrow_ix: Instruction = Instruction::new_with_bytes(
        escrow::ID, 
        &[2], 
        vec![
            AccountMeta::new(payer.pkey, true),
            AccountMeta::new_readonly(recipient.pkey, false),
            AccountMeta::new(escrow_pda, false)
        ]
    );
    let message: Message = Message::new(&[close_escrow_ix], Some(&payer.pkey));
    let mut close_escrow_tx: Transaction = Transaction::new_unsigned(message);

    // 7. sign close tx & send it
    close_escrow_tx.sign(&[&payer.keypair], latest_blockhash);
    send_tx_and_print_result(&rpc_client, &close_escrow_tx).await?;

    Ok(())
}

struct KeypairAndPKEY {
    keypair: Keypair,
    pkey: Pubkey
}

struct Accounts {
    payer: KeypairAndPKEY,
    recipient: KeypairAndPKEY
}

impl Accounts {
    /// (i know it's ugly, but this is just a test)
    fn new(payer: Keypair, recipient: Keypair) -> Self {
        let payer_pkey: Pubkey = payer.pubkey();
        let recipient_pkey: Pubkey = recipient.pubkey();
        Self {
            payer: KeypairAndPKEY { keypair: payer, pkey: payer_pkey },
            recipient: KeypairAndPKEY { keypair: recipient, pkey: recipient_pkey }
        }
    }
}

async fn init_payer_and_recipient(rpc_client: &RpcClient) -> Result<Accounts, Box<dyn std::error::Error>> {
    Ok(if std::env::var("NEW_PAYER_AND_RECIPIENT")?.parse::<bool>()? {
        // request airdrop & wait until balance tops up
        let payer: Keypair = Keypair::new();
        let recipient: Keypair = Keypair::new();
        let accs: Accounts = Accounts::new(payer, recipient);

        let airdrop_amount: u64 = LAMPORTS_PER_SOL * 5;

        let airdrop_sig: Signature = rpc_client.request_airdrop(&accs.payer.pkey, airdrop_amount).await?;
        log::info!("Sending airdrop to payer account!");
        
        loop {
            if rpc_client.confirm_transaction(&airdrop_sig).await? {
                log::info!("Received airdrop.");
                break;
            }
        }
        
        accs
    } else {
        // use accounts with given seeds
        let payer: Keypair = Keypair::from_base58_string(&std::env::var("PAYER_SEED_PHRASE")?);
        let recipient: Keypair = Keypair::from_base58_string(&std::env::var("RECIPIENT_SEED_PHRASE")?);
        Accounts::new(payer, recipient)
    })    
}

async fn send_tx_and_print_result(rpc_client: &RpcClient, tx: &Transaction) -> solana_rpc_client_api::client_error::Result<()> {
    log::info!("Sending transaction!");
    match rpc_client.send_and_confirm_transaction(tx).await {
        Ok(sig) => log::info!("Success! Tx signature: {}", sig),
        Err(e) => log::error!("Error: {}", e)
    };
    Ok(())
}