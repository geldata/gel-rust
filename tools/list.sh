#!/bin/bash
set -e -u -o pipefail

CRATE=${1:-}

# Canonicalize the path to the crate root
CRATE_ROOT=$(cd $(dirname $0)/.. && pwd)

cd $CRATE_ROOT

if [ -z "$CRATE" ]; then
    # No crate specified, use --workspace
    CRATES=$(cargo tree --workspace --depth 1 --prefix none | grep "gel-" | cut -d ' ' -f 1 | sort | uniq)
else
    # Specific crate specified, use -p $CRATE
    CRATES=$(cargo tree -p $CRATE --depth 1 --prefix none | grep "gel-" | cut -d ' ' -f 1 | sort | uniq)
fi

DEP_GRAPH=$(mktemp)
trap "rm -f $DEP_GRAPH" EXIT

for CRATE in $CRATES; do
    for DEP in $(cargo tree -p $CRATE --depth 1 --prefix none --edges=no-dev --all-features | grep "gel-" | cut -d ' ' -f 1 | sort | uniq); do
        echo "$DEP $CRATE" >> $DEP_GRAPH
    done
done

tsort $DEP_GRAPH
