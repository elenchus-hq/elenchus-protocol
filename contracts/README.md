# Contracts

Soroban smart contracts, one crate per responsibility.

| Crate | Responsibility | Status |
|---|---|---|
| `common` | Shared types (`ClaimStatus`) | skeleton |
| `claim-registry` | Anchor a manuscript by content hash | **working + tests** |
| `inquiry-escrow` | Hold and release review bounties | **testnet slice + tests** |
| `staking` | Examiner stake, lock, slash | **testnet slice + tests** |
| `reputation` | Non-transferable Elenchus score | **testnet slice + tests** |
| `cross-exam` | Weighted voting on review quality | **testnet slice + tests** |

## Prerequisites

- Rust (see `../rust-toolchain.toml`)
- [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli)

## Commands

```bash
cargo test                 # unit tests, all crates
cargo fmt --all
cargo clippy --all-targets -- -D warnings
stellar contract build     # produces wasm in target/
```

## Notes for contributors

- Read `docs/protocol-spec.md` and the related ADRs before changing contract interfaces.
- Contracts must never trust a caller-supplied address without `require_auth()`.
- Keep storage keys in a private `DataKey` enum per crate.
- If you change a public function, bump `version()` and update `packages/sdk`.
- These contracts are not audited and are not ready to hold real funds. Authority and settlement assumptions are documented in `docs/adr/0004-contract-authority-and-settlement.md`.
