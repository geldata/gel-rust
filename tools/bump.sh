#!/bin/bash
set -e -u -o pipefail

USAGE="Usage: $0 [--minor|--patch] <crate>..."

EXPECTED_TOOLS="cargo-smart-release"
for TOOL in $EXPECTED_TOOLS; do
    if ! command -v $TOOL &> /dev/null; then
        echo "💥 $TOOL could not be found"
        exit 1
    fi
done

LOG_FILE=/tmp/gel-rust-bump.log
TEMP_DIR=$(mktemp -d)
trap "echo '💥 Failed, log:' $LOG_FILE; rm -rf $TEMP_DIR" EXIT

# Parse command line arguments, defaulting to --patch if neither --minor nor
# --patch are provided. Collect the crates to bump in CRATES
MINOR=false
PATCH=false
CRATES=()

while [[ $# > 0 ]]; do
    case "$1" in
        --minor) MINOR=true ;;
        --patch) PATCH=true ;;
        *) CRATES+=("$1") ;;
    esac
    shift
done

# If no crates are provided, or if both --minor and --patch are provided,
# print the usage and exit
if [[ ${#CRATES[@]} < 1 || ($MINOR == true && $PATCH == true) ]]; then
    echo "$USAGE"
    exit 1
fi

if [[ $MINOR == true ]]; then
    VERSION_TYPE="minor"
else
    VERSION_TYPE="patch"
fi

rm -f $LOG_FILE

# Canonicalize the path to the crate root
CRATE_ROOT=$(cd $(dirname $0)/.. && pwd)

cd $CRATE_ROOT

echo "Attempting to bump crate versions:" ${CRATES[*]}

# Check out a temporary worktree for this project using a branch
# named "bump-versions" created from origin/master
git fetch origin master >> $LOG_FILE 2>&1
git worktree prune >> $LOG_FILE 2>&1
git worktree add -B bump-versions $TEMP_DIR/worktree origin/master >> $LOG_FILE 2>&1
cd $TEMP_DIR/worktree

# This incantation allows us to bump only the specified crates, and the
# references to them in other crates. Don't run any of the other magic.
cargo smart-release ${CRATES[*]} \
    --update-crates-index \
    --bump $VERSION_TYPE \
    --no-dependencies \
    --no-tag \
    --no-publish \
    --no-push \
    --no-changelog-github-release \
    --no-changelog \
    --execute >> $LOG_FILE 2>&1

git push --force origin bump-versions >> $LOG_FILE 2>&1

rm -rf $TEMP_DIR
trap - EXIT
echo "🎉 Success."
echo
echo "Create a PR at:"
echo 'https://github.com/geldata/gel-rust/compare/bump-versions?expand=1&title=Bump+versions&body=Automatically+generated+PR+to+bump+crate+versions'

