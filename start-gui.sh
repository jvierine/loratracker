#!/usr/bin/env bash
set -e
cd -- "$(dirname -- "$0")"
if ! command -v conda >/dev/null 2>&1; then
  for candidate in "$HOME/miniconda3/condabin" "$HOME/anaconda3/condabin" /opt/anaconda3/condabin /opt/miniconda3/condabin; do
    if [ -x "$candidate/conda" ]; then export PATH="$candidate:$PATH"; break; fi
  done
fi
if ! command -v conda >/dev/null 2>&1; then
  echo 'Install Miniconda first, then run setup.sh. See README.md.' >&2
  exit 1
fi
exec conda run --no-capture-output -n "${STRATOS_ENV:-base}" python scripts/launch.py "$@"
