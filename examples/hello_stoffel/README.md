# Example 1: Your First Secret Computation

Compile a StoffelLang program that adds two secret integers, inspect the compiled bytecode, and load pre-compiled `.stfb` files.

## What This Shows

- **`Stoffel::compile(source)`** compiles StoffelLang source to bytecode
- **`runtime.program().list_functions()`** inspects what functions are in the program
- **`Stoffel::load(bytecode)`** loads pre-compiled `.stfb` bytecode
- **`runtime.mpc_config()`** queries the MPC party/threshold configuration

## Key Concepts

**`secret int64`** — A value that no single party can see. Under the hood, it's split into cryptographic shares distributed across MPC servers. Operations on secrets (like `+`) work on shares without reconstructing the original value.

**Reveal by assignment** — Assigning a secret to a clear variable (`var result: int64 = sum`) triggers a reconstruction protocol where shares are combined to produce the plaintext result. This is the only way data leaves the MPC domain.

## Run

```bash
cargo run --example hello_stoffel
```

## Files

- `hello.stfl` — StoffelLang source (adds two secrets)
- `hello.stfb` — Pre-compiled bytecode
