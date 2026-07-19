#!/bin/sh
set -eu

if [ "$#" -ne 1 ]; then
  printf 'usage: %s /path/to/watch-root/price/target.toml\n' "$0" >&2
  exit 2
fi

destination=$1
destination_dir=$(dirname "$destination")
example_dir=$(CDPATH= cd -- "$(dirname "$0")" && pwd)
source_path="${example_dir}/price.json"
escaped_source_path=$(printf '%s' "$source_path" | sed 's/\\/\\\\/g; s/"/\\"/g')

mkdir -p "$destination_dir"
cat >"$destination" <<EOF
schema_name = "ffhn.target"
schema_version = 12
target_id = "price"
display_name = "Local Decimal Price"
enabled = true
escalate_after = 3
declared_type = "money"
conditions = []

[target]
kind = "file"
file_path = "${escaped_source_path}"

[fetch]
engine = "file"
max_bytes = 1024

[projection]
kind = "json_pointer"
pointer = "/price"

[type_params]
currency = "USD"
EOF
