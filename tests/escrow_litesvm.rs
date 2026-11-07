use litesvm::LiteSVM;
use litesvm_token::{
    spl_token::{self, native_mint::DECIMALS},
    CreateAssociatedTokenAccount, CreateMint, MintTo,
};
use sha2::{Digest, Sha256};
use solana_sdk::{
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address;

const ESCROW_PROGRAM: Pubkey = solana_sdk::pubkey!("HSyiRChzuxHGmg29Ezsvxyxj6tcR7wLrUySNn9NAJ1gy");
const SYSTEM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([0; 32]);

fn discriminator(name: &str) -> Vec<u8> {
    Sha256::digest(format!("global:{name}")).as_slice()[..8].to_vec()
}
fn option_pubkey(value: Option<Pubkey>) -> Vec<u8> {
    match value {
        Some(key) => {
            let mut out = vec![1];
            out.extend_from_slice(key.as_ref());
            out
        }
        None => vec![0],
    }
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

struct Fixture {
    svm: LiteSVM,
    maker: Keypair,
    taker: Keypair,
    outsider: Keypair,
    offered_mint: Pubkey,
    wanted_mint: Pubkey,
    maker_offered: Pubkey,
    maker_wanted: Pubkey,
    taker_offered: Pubkey,
    taker_wanted: Pubkey,
}
fn fixture() -> Fixture {
    let mut svm = LiteSVM::new();
    svm.add_program(ESCROW_PROGRAM, include_bytes!("../target/deploy/escrow.so"))
        .unwrap();
    let maker = Keypair::new();
    let taker = Keypair::new();
    let outsider = Keypair::new();
    for key in [&maker, &taker, &outsider] {
        svm.airdrop(&key.pubkey(), 10_000_000_000).unwrap();
    }
    let offered_mint = CreateMint::new(&mut svm, &maker)
        .authority(&maker.pubkey())
        .decimals(DECIMALS)
        .send()
        .unwrap();
    let wanted_mint = CreateMint::new(&mut svm, &maker)
        .authority(&maker.pubkey())
        .decimals(DECIMALS)
        .send()
        .unwrap();
    let maker_offered = CreateAssociatedTokenAccount::new(&mut svm, &maker, &offered_mint)
        .owner(&maker.pubkey())
        .send()
        .unwrap();
    let maker_wanted = CreateAssociatedTokenAccount::new(&mut svm, &maker, &wanted_mint)
        .owner(&maker.pubkey())
        .send()
        .unwrap();
    let taker_offered = CreateAssociatedTokenAccount::new(&mut svm, &taker, &offered_mint)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    let taker_wanted = CreateAssociatedTokenAccount::new(&mut svm, &taker, &wanted_mint)
        .owner(&taker.pubkey())
        .send()
        .unwrap();
    MintTo::new(
        &mut svm,
        &maker,
        &offered_mint,
        &maker_offered,
        2_000_000_000,
    )
    .send()
    .unwrap();
    MintTo::new(&mut svm, &maker, &wanted_mint, &taker_wanted, 2_000_000_000)
        .send()
        .unwrap();
    Fixture {
        svm,
        maker,
        taker,
        outsider,
        offered_mint,
        wanted_mint,
        maker_offered,
        maker_wanted,
        taker_offered,
        taker_wanted,
    }
}
fn offer(f: &Fixture, offer_id: u64) -> (Pubkey, Pubkey) {
    let (offer, _) = Pubkey::find_program_address(
        &[b"offer", f.maker.pubkey().as_ref(), &offer_id.to_le_bytes()],
        &ESCROW_PROGRAM,
    );
    (offer, get_associated_token_address(&offer, &f.offered_mint))
}
fn make_ix(f: &Fixture, offer_id: u64, designated: Option<Pubkey>, expires_at: i64) -> Instruction {
    let (offer, vault) = offer(f, offer_id);
    let mut data = discriminator("make");
    data.extend_from_slice(&offer_id.to_le_bytes());
    data.extend_from_slice(&1_000_000_000u64.to_le_bytes());
    data.extend_from_slice(&500_000_000u64.to_le_bytes());
    data.extend_from_slice(&option_pubkey(designated));
    data.extend_from_slice(&expires_at.to_le_bytes());
    Instruction {
        program_id: ESCROW_PROGRAM,
        accounts: vec![
            AccountMeta::new(f.maker.pubkey(), true),
            AccountMeta::new_readonly(f.offered_mint, false),
            AccountMeta::new_readonly(f.wanted_mint, false),
            AccountMeta::new(offer, false),
            AccountMeta::new(f.maker_offered, false),
            AccountMeta::new(vault, false),
            AccountMeta::new_readonly(spl_token::id(), false),
            AccountMeta::new_readonly(spl_associated_token_account::id(), false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    }
}
fn take_ix(f: &Fixture, offer_id: u64, taker: Pubkey) -> Instruction {
    let (offer, vault) = offer(f, offer_id);
    Instruction {
        program_id: ESCROW_PROGRAM,
        accounts: vec![
            AccountMeta::new(taker, true),
            AccountMeta::new(f.maker.pubkey(), false),
            AccountMeta::new_readonly(f.offered_mint, false),
            AccountMeta::new_readonly(f.wanted_mint, false),
            AccountMeta::new(offer, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(f.taker_wanted, false),
            AccountMeta::new(f.maker_wanted, false),
            AccountMeta::new(f.taker_offered, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: discriminator("take"),
    }
}
fn expire_refund_ix(f: &Fixture, offer_id: u64) -> Instruction {
    let (offer, vault) = offer(f, offer_id);
    Instruction {
        program_id: ESCROW_PROGRAM,
        accounts: vec![
            AccountMeta::new_readonly(f.outsider.pubkey(), true),
            AccountMeta::new(f.maker.pubkey(), false),
            AccountMeta::new_readonly(f.offered_mint, false),
            AccountMeta::new(offer, false),
            AccountMeta::new(vault, false),
            AccountMeta::new(f.maker_offered, false),
            AccountMeta::new_readonly(spl_token::id(), false),
        ],
        data: discriminator("expire_refund"),
    }
}

#[test]
fn make_update_restricted_take_and_atomic_settlement() {
    let mut f = fixture();
    let offer_id = 7;
    let expires_at = f.svm.get_sysvar::<Clock>().unix_timestamp + 100;
    let ix = make_ix(&f, offer_id, Some(f.taker.pubkey()), expires_at);
    send(&mut f.svm, &f.maker, &[&f.maker], ix).unwrap();
    let (offer_key, _) = offer(&f, offer_id);

    let mut update = discriminator("update");
    update.extend_from_slice(&600_000_000u64.to_le_bytes());
    update.extend_from_slice(&option_pubkey(Some(f.taker.pubkey())));
    update.extend_from_slice(&expires_at.to_le_bytes());
    send(
        &mut f.svm,
        &f.maker,
        &[&f.maker],
        Instruction {
            program_id: ESCROW_PROGRAM,
            accounts: vec![
                AccountMeta::new_readonly(f.maker.pubkey(), true),
                AccountMeta::new(offer_key, false),
            ],
            data: update,
        },
    )
    .unwrap();

    let malicious = take_ix(&f, offer_id, f.outsider.pubkey());
    assert!(send(&mut f.svm, &f.outsider, &[&f.outsider], malicious).is_err());
    let valid = take_ix(&f, offer_id, f.taker.pubkey());
    send(&mut f.svm, &f.taker, &[&f.taker], valid).unwrap();
    assert!(f.svm.get_account(&offer_key).is_none());
    let replay = take_ix(&f, offer_id, f.taker.pubkey());
    assert!(send(&mut f.svm, &f.taker, &[&f.taker], replay).is_err());
    assert_eq!(
        litesvm_token::get_spl_account::<spl_token::state::Account>(&f.svm, &f.taker_offered)
            .unwrap()
            .amount,
        1_000_000_000
    );
    assert_eq!(
        litesvm_token::get_spl_account::<spl_token::state::Account>(&f.svm, &f.maker_wanted)
            .unwrap()
            .amount,
        600_000_000
    );
}

#[test]
fn maker_can_refund_and_close_offer() {
    let mut f = fixture();
    let offer_id = 8;
    let ix = make_ix(&f, offer_id, None, 0);
    send(&mut f.svm, &f.maker, &[&f.maker], ix).unwrap();
    let (offer_key, vault) = offer(&f, offer_id);
    send(
        &mut f.svm,
        &f.maker,
        &[&f.maker],
        Instruction {
            program_id: ESCROW_PROGRAM,
            accounts: vec![
                AccountMeta::new(f.maker.pubkey(), true),
                AccountMeta::new_readonly(f.offered_mint, false),
                AccountMeta::new(offer_key, false),
                AccountMeta::new(vault, false),
                AccountMeta::new(f.maker_offered, false),
                AccountMeta::new_readonly(spl_token::id(), false),
            ],
            data: discriminator("refund"),
        },
    )
    .unwrap();
    assert!(f.svm.get_account(&offer_key).is_none());
    let after_refund = take_ix(&f, offer_id, f.taker.pubkey());
    assert!(send(&mut f.svm, &f.taker, &[&f.taker], after_refund).is_err());
    assert_eq!(
        litesvm_token::get_spl_account::<spl_token::state::Account>(&f.svm, &f.maker_offered)
            .unwrap()
            .amount,
        2_000_000_000
    );
}

#[test]
fn timed_offer_rejects_early_refund_then_refunds_after_expiry() {
    let mut f = fixture();
    let offer_id = 9;
    let mut clock = f.svm.get_sysvar::<Clock>();
    let expires_at = clock.unix_timestamp + 30;
    let ix = make_ix(&f, offer_id, None, expires_at);
    send(&mut f.svm, &f.maker, &[&f.maker], ix).unwrap();
    let (offer_key, _) = offer(&f, offer_id);
    let early = expire_refund_ix(&f, offer_id);
    assert!(send(&mut f.svm, &f.outsider, &[&f.outsider], early).is_err());

    clock.unix_timestamp = expires_at + 1;
    f.svm.set_sysvar(&clock);
    let expired = expire_refund_ix(&f, offer_id);
    send(&mut f.svm, &f.outsider, &[&f.outsider], expired).unwrap();
    assert!(f.svm.get_account(&offer_key).is_none());
    assert_eq!(
        litesvm_token::get_spl_account::<spl_token::state::Account>(&f.svm, &f.maker_offered)
            .unwrap()
            .amount,
        2_000_000_000
    );
}

#[test]
fn clock_boundary_allows_take_and_rejects_expiry_refund() {
    let mut f = fixture();
    let offer_id = 10;
    let mut clock = f.svm.get_sysvar::<Clock>();
    let expires_at = clock.unix_timestamp + 30;
    let ix = make_ix(&f, offer_id, Some(f.taker.pubkey()), expires_at);
    send(&mut f.svm, &f.maker, &[&f.maker], ix).unwrap();
    let (offer_key, _) = offer(&f, offer_id);

    clock.unix_timestamp = expires_at;
    f.svm.set_sysvar(&clock);

    let boundary_refund = expire_refund_ix(&f, offer_id);
    assert!(send(&mut f.svm, &f.outsider, &[&f.outsider], boundary_refund,).is_err());

    let boundary_take = take_ix(&f, offer_id, f.taker.pubkey());
    send(&mut f.svm, &f.taker, &[&f.taker], boundary_take).unwrap();
    assert!(f.svm.get_account(&offer_key).is_none());
    assert_eq!(
        litesvm_token::get_spl_account::<spl_token::state::Account>(&f.svm, &f.taker_offered)
            .unwrap()
            .amount,
        1_000_000_000
    );
}
