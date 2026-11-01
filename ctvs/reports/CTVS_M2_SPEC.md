# CTVS-M2 Specification: Predictive Layer

## Scope
Переход от descriptive validation к predictive validation.

## New Tests
- CTVS-011: M1 vs M2 (OOS LogLoss)
- CTVS-012: Temporal stability (blocks)
- CTVS-013: Null-model significance (p-value)
- CTVS-014: Predictive separation (E[R|state])
- CTVS-015: Policy isolation (D_t → S_t firewall)

## Required Data
- state_sequence with returns (R_t)
- train/test split by time
- temporal blocks (monthly/quarterly)

## Acceptance Criteria
- DESCRIPTIVE_CONFORMANCE = PASS
- PREDICTIVE_VALIDITY = INCONCLUSIVE (until M2 complete)

