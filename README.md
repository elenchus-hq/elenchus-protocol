# Elenchus Protocol

*Claims, cross-examined.*

In Greek philosophy, the **elenchus** is the method of testing a claim through rigorous questioning. Elenchus Protocol brings that idea to scientific peer review: an open, community-governed protocol on the [Stellar](https://stellar.org) network where reviewers are paid for rigor, held accountable for quality, and credited for their work.

> **Status: early scaffold.** `claim-registry` and the first testnet slices of escrow, staking, reputation, and cross-examination are implemented. Nothing here is audited, and nothing should hold real funds yet.

## Why

Peer review is unpaid, opaque, and invisible on a researcher's record. Elenchus makes review transparent, rewarded, and portable.

## How it works

1. An author registers a **claim** (a manuscript, anchored by its content hash) and attaches an **inquiry bounty**.
2. **Examiners** stake to claim an assignment.
3. Reviews follow a structured rubric ([`docs/review-rubric.md`](docs/review-rubric.md)).
4. The community **cross-examines** each review for quality. Soroban contracts release or slash funds accordingly.
5. Examiners build a non-transferable **Elenchus score** that they own.

Details: [`docs/protocol-spec.md`](docs/protocol-spec.md). Architecture: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Repository layout

```
contracts/    Soroban smart contracts (Rust workspace)
packages/
  tokens/     Design tokens → CSS variables, TypeScript, Tailwind preset
  schemas/    JSON Schemas for claims and reviews
  sdk/        TypeScript client for the contracts
apps/
  web/        Frontend (Vite + React)
  indexer/    Event indexer + API
docs/         Protocol spec, threat model, governance, ADRs
scripts/      Local dev, label sync, starter issues
```

## Quick start

Requirements: Node 20+ (see `.nvmrc`), [pnpm](https://pnpm.io), and for contracts Rust plus the [Stellar CLI](https://developers.stellar.org/docs/tools/cli/install-cli).

```bash
pnpm install
pnpm --filter @elenchus-protocol/tokens build
pnpm dev:web                 # http://localhost:5173
pnpm dev:indexer             # http://localhost:8787/health

cd contracts && cargo test   # contract unit tests
```

## Contributing

We want contributors on contracts, frontend, backend, docs, and research. Start with [`CONTRIBUTING.md`](CONTRIBUTING.md) and issues labeled [`good first issue`](../../labels/good%20first%20issue). Open design questions live in GitHub Discussions.

## Security

Please report vulnerabilities privately. See [`SECURITY.md`](SECURITY.md).

## License

[MIT](LICENSE)
