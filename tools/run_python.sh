#!/bin/sh
set -eu

python3_works() {
    "$@" -c 'import sys; raise SystemExit(0 if sys.version_info.major == 3 else 1)' >/dev/null 2>&1
}

if command -v python3 >/dev/null 2>&1 && python3_works python3; then
    exec python3 "$@"
fi

if command -v python >/dev/null 2>&1 && python3_works python; then
    exec python "$@"
fi

if command -v py >/dev/null 2>&1 && python3_works py -3; then
    exec py -3 "$@"
fi

printf '%s\n' 'Git safety hook requires a working Python 3 launcher (python3, python, or py -3).' >&2
exit 127
