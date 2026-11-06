# Turbin3 Week 2: Vault and Escrow

Two isolated Solana programs for the Turbin3 Builders Cohort Week 2 assignment:

- `vault`: a wallet-owned SOL vault with initialize, deposit, partial/full withdraw, and close instructions.
- `escrow`: an atomic SPL-token exchange with make, take, refund, and update instructions.
- Optional extension: offers may be restricted to one taker and assigned an expiry; expired offers can be refunded to the maker.

This repository is coursework. It does not modify or deploy the Abraxas application.

## Security model

### Vault

The vault is a PDA derived from `vault + owner`. Anchor verifies the owner signature and PDA seeds on every mutation. The program tracks deposited lamports separately from rent, prevents overdraw, uses checked arithmetic, and closes only after the tracked balance reaches zero.

### Escrow

The offer is a PDA derived from `offer + maker + offer_id`. Offered tokens move into an associated token account controlled by that PDA. `take` performs both token transfers in one Solana transaction, so either both legs settle or neither does. Account constraints bind the maker, both mints, token authorities, and vault to the stored offer. Only the maker can update or refund. A designated taker and expiry are enforced when configured.

The timed extension does not make Abraxas a custodian. In a future RWA integration, a separate settlement program could consume a valid Abraxas authorization before executing escrow; this lab intentionally keeps trust authorization and asset custody separate.

## Instructions

| Program | Instruction | Effect |
| --- | --- | --- |
| Vault | `initialize` | Creates the wallet's vault PDA. |
| Vault | `deposit(amount)` | Transfers SOL from owner to vault and records the deposit. |
| Vault | `withdraw(amount)` | Returns tracked SOL to the owner. |
| Vault | `close` | Reclaims rent after the tracked balance is zero. |
| Escrow | `make(...)` | Creates an offer and deposits the offered SPL tokens. |
| Escrow | `update(...)` | Changes wanted amount, designated taker, or expiry before settlement. |
| Escrow | `take` | Atomically exchanges wanted and offered tokens. |
| Escrow | `refund` | Maker cancels and recovers offered tokens. |
| Escrow | `expire_refund` | Anyone may trigger return to the maker after expiry. |

## Tests

The Rust integration suite loads the compiled SBF artifacts into LiteSVM. It covers:

- vault initialize, deposit, partial/full withdrawal, close, wrong-owner rejection, and overdraw rejection;
- escrow make, update, designated-taker rejection, successful atomic take, token balances, maker refund, and account closure.

Run in the pinned Anchor/Solana Linux environment:

```bash
anchor build
cargo test --workspace --all-targets -- --nocapture
```

GitHub Actions uses `solanafoundation/anchor:v0.32.1`, records the toolchain versions, runs the full suite, and prints SHA-256 hashes for both SBF artifacts. A passing CI screenshot should be added to the cohort submission only after the public run succeeds.

## Repository map

```text
programs/vault/src/lib.rs       SOL vault program
programs/escrow/src/lib.rs      SPL escrow program
tests/vault_litesvm.rs          vault lifecycle and negative tests
tests/escrow_litesvm.rs         escrow lifecycle and negative tests
.github/workflows/week2.yml     reproducible Linux build/test job
SOCIAL.md                       ready-to-post learning thread
```

## What I learned

Solana PDAs are useful for deterministic authority, but seeds alone are not a security model. Every instruction must also bind signers, mints, token authorities, amounts, and lifecycle state. Escrow is safest when exchange legs are atomic and cancellation returns funds only to the recorded maker. Timed conditions add useful recovery behavior, but they do not replace explicit authorization or independent verification.

