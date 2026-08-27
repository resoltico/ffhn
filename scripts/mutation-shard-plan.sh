#!/usr/bin/env bash
set -euo pipefail

readonly runtime_shard_total=12
readonly tooling_shard_total=4

jq -cn \
    --argjson runtime_total "$runtime_shard_total" \
    --argjson tooling_total "$tooling_shard_total" '
    def shards($scope; $total): [
        range(0; $total) as $shard
        | {
            scope: $scope,
            selector: "\($shard)/\($total)",
            artifact_name: "cargo-mutants-\($scope)-shard-\($shard)-of-\($total)"
          }
    ];
    shards("runtime"; $runtime_total) + shards("tooling"; $tooling_total)
'
