#!/usr/bin/env bash
set -eu

env_file="${DOTENV_FILE:-.env.development}"

if [ "$#" -eq 0 ]; then
  echo "Usage: $0 <command> [args...]"
  exit 64
fi

if [ -f "$env_file" ]; then
  while IFS= read -r line; do
    line="${line%$'\r'}"

    case "$line" in
      ""|[[:space:]]*|\#*)
        continue
        ;;
    esac

    if [[ "$line" == "export "* ]]; then
      line="${line#export }"
    fi

    if [[ "$line" != *"="* ]]; then
      continue
    fi

    key="${line%%=*}"
    value="${line#*=}"

    if [ -z "$key" ]; then
      continue
    fi

    if [ -v "${key}" ]; then
      continue
    fi

    if [[ "$value" == "\""*"\"" && ${#value} -ge 2 ]]; then
      value="${value:1:${#value}-2}"
    elif [[ "$value" == "'"*"'" && ${#value} -ge 2 ]]; then
      value="${value:1:${#value}-2}"
    fi

    export "$key=$value"
  done < "$env_file"
fi

exec "$@"
