# Changelog

All notable changes are recorded here. Format based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added
- Monorepo scaffold: contracts workspace, tokens, schemas, SDK, web app, indexer, docs, CI.
- `claim-registry` contract with tests.
- Design tokens with dark and light themes and automated contrast checks.
- `staking` contract with `stake`, `unlock`, `mark_unlockable`, and `slash` functions.
- Comprehensive test coverage for staking including double-stake prevention and unlock-while-locked protection.
- ADR 0004 documenting slashing authority, appeal parameters, and time limits.
