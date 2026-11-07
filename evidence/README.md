# Submission evidence

Local LiteSVM run (`cargo +1.89.0 test --workspace --all-targets`):

- 8 passed, 0 failed
- 2 vault LiteSVM tests
- 4 escrow LiteSVM tests
- 2 program `test_id` checks

Local SBF SHA-256 (Solana 2.3.0 platform-tools / Rust 1.84):

```
73366b40678b69d9143fad77b8eff81739e9094fe9538aecd3bc1b7b37ea1cf0  vault.so
3520bdd8fb052e70976f80c99f7b699bbedd9afe165eecbc5f04f8bf1977662d  escrow.so
```

CI hashes may differ slightly from a local `cargo-build-sbf` invocation; use the GitHub Actions log as the canonical public record after the green run.
