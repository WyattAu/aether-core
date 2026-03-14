#!/usr/bin/env bash
#
# wait-for-it.sh - Wait for a service to be available
#
# Usage: wait-for-it.sh host:port [--timeout=15] [-- command args]
#

set -e

TIMEOUT=15
HOST=""
PORT=""
CMD=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        *:* )
        HOST="${1%:*}"
        PORT="${1#*:}"
        shift
        ;;
        --timeout=*)
        TIMEOUT="${1#*=}"
        shift
        ;;
        --)
        shift
        CMD=("$@")
        break
        ;;
        *)
        echo "Unknown argument: $1"
        exit 1
        ;;
    esac
done

if [[ -z "$HOST" || -z "$PORT" ]]; then
    echo "Usage: wait-for-it.sh host:port [--timeout=15] [-- command args]"
    exit 1
fi

echo "Waiting for $HOST:$PORT (timeout: ${TIMEOUT}s)..."

start_ts=$(date +%s)
while true; do
    if nc -z "$HOST" "$PORT" 2>/dev/null; then
        end_ts=$(date +%s)
        echo "$HOST:$PORT is available after $((end_ts - start_ts)) seconds"
        break
    fi
    
    current_ts=$(date +%s)
    if (( current_ts - start_ts > TIMEOUT )); then
        echo "Timeout after ${TIMEOUT}s waiting for $HOST:$PORT"
        exit 1
    fi
    
    sleep 1
done

if [[ ${#CMD[@]} -gt 0 ]]; then
    exec "${CMD[@]}"
fi
