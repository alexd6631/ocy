# `--workspace` is not optional here: the root package is itself a member, so a
# bare `cargo test` silently skips every test in `ocy-core`.

default:
    @just --list

fmt:
    cargo fmt --all

fmt-check:
    cargo fmt --all --check

check:
    cargo test --workspace --all-targets --all-features --no-run

lint:
    cargo clippy --workspace --all-targets --all-features --fix --allow-dirty --allow-staged -- -D warnings

lint-check:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
    cargo test --workspace --all-features

run *ARGS:
    cargo run -- {{ARGS}}

ci: fmt-check check lint-check test

# The platform-gated paths are not exercised by a Linux test run, so at least
# keep them compiling.
check-cross:
    cargo check --workspace --all-targets --target x86_64-pc-windows-msvc
    cargo check --workspace --all-targets --target x86_64-apple-darwin

install:
    cargo install --path .

publish-all:
    cd ocy-core && cargo publish
    sleep 30
    cargo publish
