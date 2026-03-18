# Example 3: Private Salary Calculation with StoffelNetwork

Use `StoffelNetwork::builder()` for fine-grained control over the MPC network. Realistic use case: compute bonuses on a secret salary using public business rules.

## What This Shows

- **`Compiler::new().compile_source()`** for explicit compilation control
- **`StoffelNetwork::builder()`** with custom preprocessing settings
- **Loading from `.stfb`** pre-compiled bytecode into a network
- **End-to-end MPC execution** via `StoffelNetwork::execute_local()` with custom preprocessing

## Key Concepts

**When to use `StoffelNetwork` vs `Stoffel`** — `Stoffel::compile()` is the quick-start path for simple compile-and-run workflows. `StoffelNetwork::builder()` gives you control over preprocessing (Beaver triples, random shares), TLS configuration, and coordinator binding.

**MPC-safe pattern** — Branch on public values, modify secret values. In this example, `years >= 5` is a public comparison (safe to branch on), but `salary` is secret (never revealed). The bonus is applied by multiplying the secret salary by a public 0/1 flag — no secret-dependent branching needed.

**Preprocessing** — `.with_preprocessing(triples, random_shares)` tunes how many Beaver triples and random shares are pre-generated. More triples = more multiplications before needing to regenerate. Programs heavy on `*` operations need more triples.

## Run

```bash
cargo run --example network_builder
```

## Files

- `salary.stfl` — Secret salary with public business rule bonuses
- `salary.stfb` — Pre-compiled bytecode
