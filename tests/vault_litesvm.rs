use litesvm::LiteSVM;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

const VAULT_PROGRAM: Pubkey = solana_sdk::pubkey!("BctY8rmqHk5f6MJDZYmmZ78hXaqaqXMBHvr3Y52v8zK5");
const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0; 32]);

fn discriminator(name: &str) -> Vec<u8> {
    Sha256::digest(format!("global:{name}")).as_slice()[..8].to_vec()
}

fn send(
    svm: &mut LiteSVM,
    payer: &Keypair,
    signers: &[&Keypair],
    ix: Instruction,
) -> Result<(), String> {
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        signers,
        svm.latest_blockhash(),
    );
    let result = svm
        .send_transaction(tx)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"));
    svm.expire_blockhash();
    result
}

#[test]
fn vault_initialize_deposit_withdraw_and_close() {
    let mut svm = LiteSVM::new();
    svm.add_program(VAULT_PROGRAM, include_bytes!("../target/deploy/vault.so"))
        .unwrap();
    let owner = Keypair::new();
    svm.airdrop(&owner.pubkey(), 5_000_000_000).unwrap();
    let (vault, _) =
        Pubkey::find_program_address(&[b"vault", owner.pubkey().as_ref()], &VAULT_PROGRAM);

    send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            ],
            data: discriminator("initialize"),
        },
    )
    .unwrap();

    let deposit = 1_000_000_000u64;
    let mut deposit_data = discriminator("deposit");
    deposit_data.extend_from_slice(&deposit.to_le_bytes());
    send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            ],
            data: deposit_data,
        },
    )
    .unwrap();
    let after_deposit = svm.get_account(&vault).unwrap().lamports;

    let withdrawal = 400_000_000u64;
    let mut withdraw_data = discriminator("withdraw");
    withdraw_data.extend_from_slice(&withdrawal.to_le_bytes());
    send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
            ],
            data: withdraw_data,
        },
    )
    .unwrap();
    assert_eq!(
        after_deposit - svm.get_account(&vault).unwrap().lamports,
        withdrawal
    );

    let mut remainder_data = discriminator("withdraw");
    remainder_data.extend_from_slice(&(deposit - withdrawal).to_le_bytes());
    send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
            ],
            data: remainder_data,
        },
    )
    .unwrap();
    send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
            ],
            data: discriminator("close"),
        },
    )
    .unwrap();
    assert!(svm.get_account(&vault).is_none());
    let closed_again = send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
            ],
            data: discriminator("close"),
        },
    );
    assert!(closed_again.is_err());
}

#[test]
fn vault_rejects_wrong_owner_and_overdraw() {
    let mut svm = LiteSVM::new();
    svm.add_program(VAULT_PROGRAM, include_bytes!("../target/deploy/vault.so"))
        .unwrap();
    let owner = Keypair::new();
    let attacker = Keypair::new();
    svm.airdrop(&owner.pubkey(), 2_000_000_000).unwrap();
    svm.airdrop(&attacker.pubkey(), 2_000_000_000).unwrap();
    let (vault, _) =
        Pubkey::find_program_address(&[b"vault", owner.pubkey().as_ref()], &VAULT_PROGRAM);
    send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            ],
            data: discriminator("initialize"),
        },
    )
    .unwrap();

    let mut data = discriminator("withdraw");
    data.extend_from_slice(&1u64.to_le_bytes());
    let wrong_owner = send(
        &mut svm,
        &attacker,
        &[&attacker],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(attacker.pubkey(), true),
                AccountMeta::new(vault, false),
            ],
            data: data.clone(),
        },
    );
    assert!(wrong_owner.is_err());

    let overdraw = send(
        &mut svm,
        &owner,
        &[&owner],
        Instruction {
            program_id: VAULT_PROGRAM,
            accounts: vec![
                AccountMeta::new(owner.pubkey(), true),
                AccountMeta::new(vault, false),
            ],
            data,
        },
    );
    assert!(overdraw.is_err());
}
