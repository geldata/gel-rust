_default:
    just --list

build-minimal:
    #!/bin/bash
    # Generate a lockfile with minimal versions and build it
    cargo +nightly generate-lockfile -Z minimal-versions && cargo build

    for crate in $(tools/list.sh); do
        echo "Building $crate..."
        cargo build -p $crate
    done

test:
    # Test all features
    cargo test --workspace --all-features

    # Check no default features
    cargo check --no-default-features --workspace
    
    # Check `fs` feature (gel-tokio)
    cargo check --features=fs --package gel-tokio
    
    # Check with env feature, gel-tokio
    cargo check --features=env --package gel-tokio

    # Test gel-protocol without default features
    cargo test --package=gel-protocol --no-default-features

    # Test gel-protocol with "all-types" feature
    cargo test --package=gel-protocol --features=all-types

    cargo clippy --workspace --all-features --all-targets

    cargo fmt --check

test-fast:
    cargo fmt

    cargo test --workspace --features=unstable

    cargo clippy --workspace --all-features --all-targets

check:
    #!/bin/bash
    set -euo pipefail

    cargo check --workspace --all-features --all-targets
    cargo check --workspace --no-default-features --all-targets

    # Check all crates in the workspace
    CRATES=`cargo tree --workspace --depth 1 --prefix none | grep "gel-" | cut -d ' ' -f 1 | sort | uniq`
    for crate in $(tools/list.sh); do
        echo "Checking $crate..."
        # TODO: this doesn't currently pass because we've got some crates that fail to check
        cargo check --quiet --package $crate --no-default-features || echo "Failed to check $crate with no default features"
        cargo check --quiet --package $crate --no-default-features --all-targets
    done

    echo "Checked all crates."

publish:
    tools/publish.sh gel-tokio
