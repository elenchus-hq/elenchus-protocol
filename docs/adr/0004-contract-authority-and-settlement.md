# ADR 0004: Contract authority and settlement parameters

- Status: Proposed for testnet
- Date: 2026-10-08

## Context

Escrow releases, stake slashing, reputation updates, and quality-vote eligibility all require trusted settlement inputs. The draft interfaces leave the authorizing contract and vote rules open. The protocol has no deployed router or on-chain governance contract yet.

## Decision

- Escrow, staking, reputation, and cross-exam are initialized once with an authority address that must authorize initialization. The address is immutable after initialization. For testnet it is a maintainer-controlled governance multisig; production deployment must use an accepted governance address.
- Escrow `release` and `refund_remaining`, staking `slash` and `unlock`, reputation `record_outcome`, and cross-exam electorate/review administration require that configured authority's authorization. Examiners authorize their own stake and voters authorize their own commits and reveals. Funders authorize deposits.
- Escrow accepts one SAC token deposit per claim. The settlement authority alone decides release amounts and when a failed milestone warrants returning the unspent balance to the original funder. A claim can only be refunded once.
- The staking authority may slash each assignment once, with `bps` constrained by both 10,000 and the nonzero maximum fixed at initialization. It may unlock the remaining stake only after settlement. An examiner cannot unilaterally unlock a live assignment.
- A proposed slash is subject to a seven-day off-chain appeal window. Governance resolves any appeal before calling `slash`; the authority should execute the final decision within 30 days. These are governance obligations, not contract-enforced clocks in v0.
- The cross-exam authority allowlists voters as authors, examiners, or editors and registers each review's author and field. The registered review author cannot vote on that review. Reputation is snapshotted at commit time; non-positive score gives no voting eligibility, and positive weight is capped at 1,000,000 per voter.
- Revealed vote weight is capped at 1,000 reputation-weight units per group (`author`, `examiner`, `editor`) per review. The tally first computes each group's weighted-average quality, then applies that group's cap. Group caps are absolute, not percentages, so an absent group does not donate voting weight to another group.
- Each review has an immutable commit and reveal period set at contract initialization. Commit is allowed through its deadline; reveal is allowed strictly after that deadline and through the reveal deadline. Quality is an integer from 0 through 100. The commitment is SHA-256 over the Soroban XDR encoding of `(review_id, voter, quality, salt)`, binding it to both the review and voter.
- Reputation fields are Soroban `Symbol` values. They are case-sensitive opaque identifiers; callers must use the same canonical lowercase `snake_case` symbol everywhere. Contracts do not normalize field names.
- Persistent contract entries use a 518,400-ledger TTL threshold and a 518,400-ledger target remaining lifetime when written (approximately 30 days at five seconds per ledger).

## Consequences

- A single authority is a temporary testnet trust assumption and a centralization risk. Deployments must initialize atomically with controlled governance addresses.
- Appeal eligibility and the slash execution window depend on governance process until an assignment lifecycle/router can enforce them on-chain.
- The absolute group caps prevent any one group from contributing more than 1,000 effective weight units, while leaving cross-group composition dependent on voter turnout.
- Reviews accept at most 100 commits so bounded tallying stays within predictable contract resource limits.
- Field spelling is part of the reputation key; changing a field name requires an explicit migration or a new field.
