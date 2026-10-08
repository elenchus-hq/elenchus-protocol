# Protocol specification (draft)

Status: **draft**. Sections marked *Open* are decided in GitHub Discussions, then recorded as ADRs.

## Actors

- **Author** registers a claim and funds its inquiry bounty.
- **Examiner** stakes to review a claim and is paid for accepted reviews.
- **Reader** reads claims, reviews, and verdicts. No account needed.
- **Maintainers/Governance** set protocol parameters (see `governance.md`).

## Claim lifecycle

```
pending ──► examining ──► corroborated
                     ├──► contested
                     └──► refuted
```

| State | Meaning |
|---|---|
| `pending` | Registered, bounty funded, not enough examiners yet |
| `examining` | Required number of examiners have staked |
| `corroborated` | Reviews accepted, majority recommend corroboration |
| `contested` | Reviews disagree materially |
| `refuted` | Reviews accepted, majority recommend refutation |

A verdict says what examiners concluded. It is not a statement of truth.

## Flow

1. **Register.** `claim-registry.register_claim(author, content_hash, cid)`.
2. **Fund.** Author deposits USDC into `inquiry-escrow` for the claim.
3. **Stake.** Examiners lock stake in `staking` to claim an assignment.
4. **Review.** Examiner submits a review (JSON validating against `review.schema.json`) to IPFS and records its hash.
5. **Cross-examine.** Authors, other examiners, and editors vote on review quality in `cross-exam`, weighted by reputation.
6. **Settle.** Accepted reviews are paid from escrow and stake returned. Rejected reviews forfeit part of stake. Reputation updates.

## Draft interfaces

The testnet contracts implement the following v0 interfaces. Their proposed authorization and timing choices are recorded in [ADR 0004](adr/0004-contract-authority-and-settlement.md).

```rust
// inquiry-escrow
fn initialize(authority: Address);
fn deposit(env, claim_id: BytesN<32>, funder: Address, token: Address, amount: i128);
fn release(env, claim_id: BytesN<32>, examiner: Address, amount: i128); // restricted
fn refund_remaining(env, claim_id: BytesN<32>) -> i128; // restricted

// staking
fn initialize(authority: Address, token: Address, treasury: Address, max_slash_bps: u32);
fn stake(env, examiner: Address, claim_id: BytesN<32>, amount: i128);
fn unlock(env, examiner: Address, claim_id: BytesN<32>) -> i128; // restricted
fn slash(env, examiner: Address, claim_id: BytesN<32>, bps: u32) -> i128; // restricted

// reputation
fn initialize(authority: Address);
fn score(env, examiner: Address, field: Symbol) -> i128;
fn record_outcome(env, examiner: Address, field: Symbol, delta: i128); // restricted

// cross-exam
fn initialize(admin: Address, reputation: Address, commit_period: u64, reveal_period: u64);
fn set_voter_group(voter: Address, group: VoterGroup); // restricted
fn register_review(review_id: BytesN<32>, author: Address, field: Symbol); // restricted
fn commit(voter: Address, review_id: BytesN<32>, hash: BytesN<32>);
fn reveal(voter: Address, review_id: BytesN<32>, quality: u32, salt: BytesN<32>);
fn tally(env, review_id: BytesN<32>) -> Tally;
```

`Tally` contains `weighted_quality`, `total_weight`, and `revealed_votes`; weighted mean quality is `weighted_quality / total_weight` when the total is nonzero. Quality ranges from 0 to 100. A commitment is SHA-256 of the XDR encoding of `(review_id, voter, quality, salt)`. The voter must be authorized for both commit and reveal. Vote weight is the positive reputation score, capped at 1,000,000 per voter; each allowlisted voter group has a 1,000-unit cap per review. Each review accepts at most 100 commits.

`claim-registry.register_claim` emits `claim_registered` with the content hash and author. Persistent entries use a 518,400-ledger TTL threshold and are extended to a 518,400-ledger target remaining lifetime when written.

## Open questions

1. **Who decides review quality?** For testnet, the governance authority allowlists authors, peer examiners, and editors, and caps each group's effective contribution. See ADR 0004.
2. **Reputation scope.** Field-scoped scores are more meaningful but fragment the system. How are fields defined?
3. **Anonymity.** Blind, open, or examiner's choice? Open identity builds trust. Anonymity protects junior reviewers.
4. **Bounty funding.** Author-paid, funder-paid, institution-paid, or a community pool?
5. **Slashing parameters.** The testnet maximum is fixed at initialization; a seven-day appeal window and 30-day execution window are governance obligations. See ADR 0004.
6. **Identity and Sybil resistance.** ORCID links? Stake size? Both?
7. **Cross-contract authority.** Testnet contracts use immutable authority addresses set at initialization. Production router/governance integration remains open.
