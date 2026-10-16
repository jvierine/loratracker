#!/usr/bin/env bash
set -e
cd -- "$(dirname -- "$0")"
conda env update -n "${STRATOS_ENV:-base}" -f environment.yml
printf '\nRuntime installed. Install TeX Live/MacTeX for plots; see README.md.\nStart with ./start-gui.sh (or double-click start-gui.command).\n'
