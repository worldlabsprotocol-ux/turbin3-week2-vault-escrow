# X thread draft

1/ Built a Solana vault and SPL-token escrow for Turbin3 Week 2.

The goal was not just to make tokens move. It was to make authority, settlement, cancellation, and recovery explicit and testable.

2/ The vault supports initialize, deposit, withdraw, and close.

Each vault is a PDA bound to its owner. Only that wallet can withdraw, tracked deposits cannot be overdrawn, and rent is reclaimed only after the balance is cleared.

3/ The escrow supports make, take, refund, and update.

The maker deposits one SPL token and defines what they want in return. Taking the offer transfers both sides atomically: both settle, or neither does.

4/ I also added a timed condition and optional designated taker.

An expired offer cannot be taken. Its assets can be returned to the maker, while a restricted offer rejects every other wallet.

5/ The main lesson: PDAs provide deterministic program authority, but security comes from binding every relevant account and state transition.

Signer, mint, token authority, amount, expiry, and lifecycle all need enforcement.

6/ The programs are tested with LiteSVM, including failure paths: wrong owner, overdraw, wrong taker, refund, account closure, and final token balances.

7/ This also connects to the RWA thesis behind Abraxas.

Abraxas can verify whether an action is authorized. A separate escrow can execute settlement after that authorization is consumed. Trust infrastructure should not automatically become the custodian.

8/ That separation matters for tokenized real estate, private credit, and institutional assets: evidence and authorization can remain portable while settlement stays enforceable onchain.

Repository: [ADD PUBLIC REPOSITORY URL]

