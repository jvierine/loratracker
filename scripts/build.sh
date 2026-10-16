#!/usr/bin/env bash
set -e
cd -- "$(dirname -- "$0")/.."
exec conda run --no-capture-output -n "${STRATOS_ENV:-base}" python scripts/build.py "$@"
