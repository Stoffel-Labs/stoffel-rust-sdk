# Example 2: Secret Multiply and Reveal

Compile a program with secret multiplication and explore different MPC configurations.

## What This Shows

- **MPC configuration validation** — the SDK enforces `n >= 3t + 1` at build time
- **Multiple configurations** — 5/1 (standard), 7/2 (higher tolerance), 3/1 (rejected)
- **Bytecode determinism** — source-compiled and pre-compiled bytecode are identical
- **Full MPC execution** — multiplication triggers the real Beaver triple protocol across all 5 parties

## Key Concepts

**Addition vs. multiplication on secrets** — Addition of secret-shared values is "free" (local share operation, no network traffic). Multiplication requires a network round using Beaver triples — pre-generated random values that let parties compute products without revealing inputs.

**Parties and threshold** — `parties` is the number of MPC servers. `threshold` is the maximum number that can be malicious (Byzantine). HoneyBadger requires `n >= 3t + 1`:
- 5 parties, threshold 1: tolerates 1 malicious party
- 7 parties, threshold 2: tolerates 2 malicious parties
- 4 parties, threshold 1: minimum valid configuration

## Run

```bash
cargo run --example secret_arithmetic
```

## Files

- `secret_mul.stfl` — Secret price * quantity with tax calculation
- `secret_mul.stfb` — Pre-compiled bytecode
