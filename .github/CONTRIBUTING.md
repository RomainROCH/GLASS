# Contributing

Thanks for helping improve this generic overlay framework base.

## Build

Use Rust stable and build from the repository root:

```sh
# Full workspace
cargo build --workspace

# Starter/minimal harness only
cargo build -p glass-starter
```

## Run the minimal example

Run the starter harness:

```sh
cargo run -p glass-starter
```

On first run, a default `config.ron` is generated automatically.

## Submit a bug report

Open a GitHub issue and include:

- clear expected vs actual behavior
- minimal reproduction steps
- OS + Rust toolchain version
- relevant logs/error output
- config snippet (if applicable)

## Propose a feature

Open a GitHub issue with:

- problem statement and use case
- proposed API/config shape
- scope (which crate/module is affected)
- compatibility or migration notes

## Coding style expectations

- run `cargo fmt --all` before submitting
- run `cargo clippy --workspace -- -D warnings`
- keep changes focused and minimal
- avoid unrelated refactors in the same PR
