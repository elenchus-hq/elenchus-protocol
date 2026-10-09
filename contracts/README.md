# Contracts

Soroban smart contracts, one crate per responsibility.

| Crate | Responsibility | Status |
|---|---|---|
| `common` | Shared types (`ClaimStatus`) | skeleton |
| `claim-registry` | Anchor a manuscript by content hash | **working + tests** |
| `inquiry-escrow` | Hold and release review bounties | stub |
| `staking` | Examiner stake, lock, slash | **working + tests** |
| `reputation` | Non-transferable Elenchus score | stub |
| `cross-exam` | Weighted voting on review quality | stub |

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

- Each stub has a matching `good first issue` describing the intended interface. Read `docs/protocol-spec.md` first.
- Contracts must never trust a caller-supplied address without `require_auth()`.
- Keep storage keys in a private `DataKey` enum per crate.
- If you change a public function, bump `version()` and update `packages/sdk`.
