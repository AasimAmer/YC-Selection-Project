# FAULTLINE

FAULTLINE will answer which TypeScript and JavaScript consumers may be affected by a source change and show the evidence for each result. This repository is at the contract review stage. It includes nine Rust crates, a fixture-backed empty engine, response schemas and grammar smoke tests. The real engine currently returns an explicit unavailable error.

## Development

Install the Rust toolchain specified in `rust-toolchain.toml` and a C compiler. From the checkout run:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -q -p faultline-contract --example schema | diff - fixtures/bootstrap/schemas.json
```

Review the owned types and five method signatures in `crates/faultline-contract` against the bootstrap fixtures. Both developers review the wire format and passing GitHub checks before tagging `contract-v1`. Developer A then takes parser and graph packets. Developer B starts with the [handoff prompt](docs/DEVELOPER_B_HANDOFF.md).

[Product blueprint](docs/01_FAULTLINE_PROJECT_BLUEPRINT.md) · [Execution playbook](docs/02_TWO_PERSON_EXECUTION_PLAYBOOK.md) · [Repository instructions](AGENTS.md)
