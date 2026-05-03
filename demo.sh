#!/bin/sh
set -e

DEMOS="map inc_list and make_tree"

usage() {
    echo "Usage: $0 [demo]"
    echo "  demo: one of: $DEMOS"
    echo "  (no argument runs all demos)"
    exit 1
}

run_demo() {
    name=$1
    echo "── $name ──────────────────────────────────────"
    cc -std=c11 -o "out/$name" "out/$name.c" 2>&1
    "./out/$name"
}

# Regenerate all .c files once
cargo run -q 2>/dev/null

if [ $# -eq 0 ]; then
    for name in $DEMOS; do
        run_demo "$name"
    done
elif [ $# -eq 1 ]; then
    case " $DEMOS " in
        *" $1 "*) run_demo "$1" ;;
        *) echo "Unknown demo '$1'"; usage ;;
    esac
else
    usage
fi
