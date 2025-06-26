#!/bin/bash
set -e -u -o pipefail

USAGE="Usage: $0 [--new] <crate>"

NEW_CRATE=0

if [ "$1" = "--new" ]; then
    NEW_CRATE=1
    shift
fi

CRATE=${1:-}
EXPECTED_TOOLS="cargo-download cargo-info"
for TOOL in $EXPECTED_TOOLS; do
    if ! command -v $TOOL &> /dev/null; then
        echo "💥 $TOOL could not be found"
        exit 1
    fi
done

LOG_FILE=/tmp/gel-rust-publish.log
TEMP_DIR=$(mktemp -d)

if [ $# -gt 1 ]; then
    echo "$USAGE"
    exit 1
fi

trap "echo '💥 Failed, log:' $LOG_FILE; rm -rf $TEMP_DIR" EXIT
rm -f $LOG_FILE

# Canonicalize the path to the crate root
CRATE_ROOT=$(cd $(dirname $0)/.. && pwd)

cd $CRATE_ROOT

CRATE_ORDER=$(./tools/list.sh $CRATE)

# Check out a temporary worktree for this project from origin/master
git fetch origin master >> $LOG_FILE 2>&1
git worktree add $TEMP_DIR/worktree origin/master >> $LOG_FILE 2>&1
cd $TEMP_DIR/worktree

cargo metadata --format-version 1 > $TEMP_DIR/metadata.json 2> /dev/null

if [ $NEW_CRATE -eq 1 ]; then
    NEEDS_PUBLISH+=($CRATE)
else
    # Step 1: Ensure that all crates with differences from the published version
    # have a different version

    if [ -z "$CRATE" ]; then
        echo "Checking publication state for all crates"
    else
        echo "Checking publication state for:" $CRATE_ORDER
    fi

    # Collect crates that need bump or publish
    NEEDS_BUMP=()
    NEEDS_PUBLISH=()
    COMPARE_DIR="$TEMP_DIR/compare"

    mkdir -p target/package-cache

    for CRATE in $CRATE_ORDER; do
        printf "\n\n*** $CRATE ***\n\n" >> $LOG_FILE 2>&1

        CRATE_VERSION=$(jq -r ".packages[] | select(.name == \"$CRATE\") | .version" $TEMP_DIR/metadata.json)

        cargo download $CRATE -o target/package-cache/$CRATE-latest.crate >> $LOG_FILE 2>&1 || (
            echo "💥 Failed to download $CRATE!"
            echo "If this is a new crate, run \`$0 --new $CRATE\`"
            echo
            exit 1
        )

        rm -rf "$COMPARE_DIR" || true
        mkdir -p "$COMPARE_DIR/a" "$COMPARE_DIR/b"

        tar --strip-components=1 -xvf target/package-cache/$CRATE-latest.crate -C "$COMPARE_DIR/a" >> $LOG_FILE 2>&1

        PUBLISHED_VERSION=$(cat "$COMPARE_DIR/a/Cargo.toml" | grep "^version" | head -n 1 | cut -d '=' -f 2 | tr -d '" ')
        DIFF_FILE=/tmp/${CRATE}.diff
    
        # If the versions don't match, we ask the user to publish the crate
        if [ "$CRATE_VERSION" != "$PUBLISHED_VERSION" ]; then
            echo "  ❌ $CRATE: git version ($CRATE_VERSION) is different from published version ($PUBLISHED_VERSION)"
            NEEDS_PUBLISH+=($CRATE)
        else
            # Don't package the crate unless the version hasn't changed
            cargo package -p $CRATE --no-verify >> $LOG_FILE 2>&1
            tar --strip-components=1 -xvf target/package/$CRATE-$CRATE_VERSION.crate -C "$COMPARE_DIR/b" >> $LOG_FILE 2>&1
            if diff -u --exclude=.cargo_vcs_info.json --exclude=Cargo.lock -r "$COMPARE_DIR/a" "$COMPARE_DIR/b" > $DIFF_FILE; then
                echo "  ✅ $CRATE: up to date"
            else
                echo "  ❌ $CRATE: unpublished changes from $PUBLISHED_VERSION (See $DIFF_FILE)"
                NEEDS_BUMP+=($CRATE)
            fi
        fi
    done

    cd $CRATE_ROOT

    if [ ${#NEEDS_BUMP[@]} -gt 0 ]; then
        echo
        echo "The following crates need to be bumped:"
        for CRATE in "${NEEDS_BUMP[@]}"; do
            echo "  $CRATE"
        done
        echo
        echo "To fix, run the following command and then re-run this script:"
        echo
        echo "tools/bump.sh --patch ${NEEDS_BUMP[@]}"
        echo
        echo "Should I run this for you? (y/N)"
        read -n 1 -s
        if [ "$REPLY" = "y" ]; then
            echo "Yes"
            echo
            exec ./tools/bump.sh --patch ${NEEDS_BUMP[@]}
        fi
        exit 1
    fi
fi

if [ ${#NEEDS_PUBLISH[@]} -gt 0 ]; then
    echo
    echo "The following crates need to be published:"
    for CRATE in "${NEEDS_PUBLISH[@]}"; do
        echo "  $CRATE"
    done

    echo
    echo "📢 I'm going to publish these crates now, prompting for each one."
    echo "📢 It is safe to cancel at any time (using ctrl-c) and resume the process later."

    for CRATE in "${NEEDS_PUBLISH[@]}"; do
        CRATE_VERSION=$(jq -r ".packages[] | select(.name == \"$CRATE\") | .version" $TEMP_DIR/metadata.json)
        COMMAND="git tag --force releases/$CRATE/v$CRATE_VERSION origin/master && git push --force origin releases/$CRATE/v$CRATE_VERSION"
        echo
        echo "📦 $CRATE"
        echo "----------------------------------------"
        echo "$COMMAND"
        echo "Run? (ctrl-c to cancel or any other key to continue)"
        # Clear input buffer so we don't accidentally run the command
        while read -t 0; do
            read -t 0
        done
        read -n 1 -s
        eval $COMMAND >> $LOG_FILE 2>&1

        echo "🫸 Pushing tag to trigger CI..."
        echo "⌛️ Actions status: https://github.com/geldata/gel-rust/actions/workflows/publish-crate.yaml"

        # Wait for crate to be published. Parse "Version:" from `cargo info` and
        # wait until it matches CRATE_VERSION.
        while ! cargo info $CRATE@$CRATE_VERSION --registry crates-io > /dev/null 2>&1; do
            echo "Waiting for $CRATE@$CRATE_VERSION to be published (this may take a while)..."
            sleep 15
        done

        cargo info $CRATE@$CRATE_VERSION --registry crates-io

        echo "✅ $CRATE published!"
    done
else
    echo "🎉 No crates need to be published"
fi

rm -rf $TEMP_DIR
trap - EXIT
echo "🎉 Success"
