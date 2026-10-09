# Threat model (starter)

Status: **starter**. This is a research contribution target. Extend it.

| Threat | Example | Candidate mitigations |
|---|---|---|
| **Sybil examiners** | One person registers many wallets to farm bounties or sway votes | Stake requirement, identity attestations (e.g. ORCID), reputation-weighted voting, rate limits |
| **Review rings** | Groups approve each other's reviews | Conflict-of-interest declarations, graph analysis of voter overlap, random voter selection |
| **Collusion with author** | Author bribes examiners for a favorable verdict | Blind assignment, slashing for outcome-correlated payments, randomized examiner pools |
| **Low-effort reviews** | Short generic reviews to collect bounty | Rubric with required evidence, quality voting, reproduction bonus, slashing |
| **Plagiarized reviews** | Copy of another review | Content-hash comparison, similarity checks off-chain, slashing |
| **Vote manipulation** | Buying or coercing quality votes | Reputation weighting, vote caps per group, commit-reveal voting |
| **Griefing authors** | Rejecting valid claims to harm an author | Multiple examiners, appeal path, reputation cost for overturned reviews |
| **Escrow drain** | Contract bug releases funds incorrectly | Small separate contracts, audits, caps on bounty size during early phases (governance-settable maximum bounty enforced on deposit) |
| **Front-running** | Seeing a review before it is final | Commit-reveal for review hashes |
| **Manuscript tampering** | Changing a file after registration | Content hash anchored on-chain, CID verification |
| **Key compromise** | Examiner or author key stolen | Standard wallet hygiene, recovery out of scope for v0 |
| **Censorship at the gateway** | IPFS gateway hides a manuscript | Multiple gateways, pinning guidance |

## Assumptions

- Stellar consensus and Soroban execution are correct.
- Wallet software signs only what the user approves.
- Authors can bring their own manuscripts. The protocol does not verify authenticity of the underlying science.

## Out of scope for v0

Legal and regulatory questions about tokenized rewards, jurisdiction-specific research-ethics requirements, and recovery of lost keys.