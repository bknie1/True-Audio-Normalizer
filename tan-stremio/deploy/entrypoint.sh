#!/bin/sh
# Launches the TAN Stremio proxy from environment variables.
set -e

if [ -z "$TAN_UPSTREAM" ]; then
  echo "TAN_UPSTREAM is required (your debrid Torrentio Install URL)." >&2
  exit 1
fi

set -- proxy --upstream "$TAN_UPSTREAM" --port 5870 --bind 0.0.0.0 --hls
[ -n "$TAN_SECRET" ]  && set -- "$@" --secret  "$TAN_SECRET"
[ -n "$TAN_PROFILE" ] && set -- "$@" --profile "$TAN_PROFILE"

exec /app/tan-stremio "$@"
