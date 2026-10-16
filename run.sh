#!/usr/bin/env bash
set -e
cd -- "$(dirname -- "$0")"
# Compatibility entry point. Data review is the default; --live enables the SDR.
if [ "${STATION_REVIEW:-1}" = 0 ]; then exec ./start-gui.sh --live "$@"; fi
exec ./start-gui.sh "$@"
