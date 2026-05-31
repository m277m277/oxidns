set positional-arguments

# Run the standard local quality gate without mutating source files.
check:
    cargo +nightly fmt --all --check
    cargo +nightly clippy --all-targets --all-features -- -D warnings
    cargo test

# Apply formatting and Clippy autofixes where available.
fix:
    cargo +nightly fmt --all
    cargo +nightly clippy --all-targets --all-features --fix --allow-dirty --allow-staged -- -D warnings

# Fast iteration path when full tests are unnecessary.
lint:
    cargo +nightly fmt --all --check
    cargo +nightly clippy --all-targets --all-features -- -D warnings

# Install the repository-managed Git hooks directory for this clone.
install-hooks:
    git config core.hooksPath .githooks

# Build feature bundles defined in Cargo.toml. Useful for verifying that
# every feature combination still compiles after changes to module gating.
check-minimal:
    cargo +nightly clippy --no-default-features --features minimal --all-targets -- -D warnings

check-standard:
    cargo +nightly clippy --no-default-features --features standard --all-targets -- -D warnings

check-full:
    cargo +nightly clippy --all-features --all-targets -- -D warnings

# Verify all three bundles still compile cleanly and run the test suite
# against the default (full) feature set. Integration tests reference
# feature-gated plugins, so they only execute under --features full.
check-matrix: check-minimal check-standard check-full
    cargo test --all-features

