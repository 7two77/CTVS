# Empirical Data Directory

## Purpose
Contains REAL market state observations with future returns.

## Format
Each file is JSONL (JSON Lines):
{"timestamp": 1704067200, "state": "S0", "return_forward": 0.00123}

## Rules
1. `return_forward` is the ACTUAL future return, not synthetic.
2. Data must be time-ordered.
3. No look-ahead: `return_forward` at time T is R_{T+1}.
4. States are estimated from features at time T ONLY.

## Status
- EMPTY: no empirical data provided yet.
- DO NOT use synthetic fixtures here.

