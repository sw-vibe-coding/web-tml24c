#!/usr/bin/env bash
set -euo pipefail

PORT=9135

exec trunk serve --port "$PORT" "$@"
