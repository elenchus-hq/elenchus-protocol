# ADR 0004: Slashing authority and appeal parameters

- Status: Accepted
- Date: 2026-10-09

## Context

The staking contract must slash examiner stake for rejected reviews or no-shows. This raises questions:

1. **Who may call `slash`?** The staking contract cannot independently judge review quality.
2. **Appeal path.** Slashing is punitive. False positives hurt participation.
3. **Time limits.** Stake cannot remain locked indefinitely if the claim stalls.

## Decision

### Slash authority

The `slash` function is **restricted** to a single authorized address, configured at contract initialization. In v0, this is the **cross-exam contract** address, which tallies reputation-weighted votes on review quality.

Future governance may change the authority to a multisig or DAO.

### Slashing parameters

- **Maximum slash per incident:** 50% (5000 basis points), enforced by `MAX_SLASH_BPS`.
- **Basis points validation:** All slash calls must specify `bps ≤ 10000`. Values above `MAX_SLASH_BPS` are rejected.
- **Cumulative slashing:** Multiple infractions may be slashed separately (e.g., 20% for a no-show, 50% for plagiarism).

### Appeal and time limits

- **Appeal path (v0):** Not implemented. The cross-exam vote is final. A governance override (via slash authority re-assignment) is the only recourse.
- **Review deadline:** 14 days. If cross-exam does not finalize a vote within 14 days of review submission, the staking contract's `mark_unlockable` function may be called by anyone, allowing the examiner to unlock their stake without slashing.
- **Assignment expiry:** 30 days. If an examiner stakes but never submits a review, after 30 days the slash authority may call `slash` for a no-show (default: 20% = 2000 bps) or call `mark_unlockable` to allow re-assignment.

## Consequences

### Positive
- Clear authority model: one address (cross-exam contract) decides slashing in v0.
- Bounded penalties: no single incident can slash more than 50%.
- Time-bound locks: stake cannot be held hostage if a claim or vote stalls.

### Negative
- No on-chain appeal in v0. Wrongly slashed examiners must escalate off-chain.
- Hardcoded time limits. These should become governance parameters.
- Single point of trust in the slash authority. A compromised cross-exam contract could slash arbitrarily (within MAX_SLASH_BPS).

### Future work
- Add appeal window (e.g., 7-day challenge period before slash is finalized).
- Move time limits and MAX_SLASH_BPS to on-chain governance parameters.
- Emit events for all slashing and unlocking actions for off-chain transparency.

## References

- `docs/protocol-spec.md` § Open questions #5 (slashing parameters)
- `docs/threat-model.md` (low-effort reviews, plagiarism, griefing)
- Issue #11 (staking implementation)
