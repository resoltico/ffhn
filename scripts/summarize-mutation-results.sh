#!/usr/bin/env bash
set -euo pipefail

if (( $# != 3 )); then
    printf 'usage: %s <shard-plan-json> <artifact-root> <step-summary>\n' "$0" >&2
    exit 1
fi

shard_plan_json="$1"
artifact_root="$2"
step_summary="$3"

if ! jq -e '
    def selector_parts:
        capture("^(?<index>[0-9]+)/(?<total>[1-9][0-9]*)$")
        | {index: (.index | tonumber), total: (.total | tonumber)};
    def artifact_parts:
        capture("^cargo-mutants-(?<scope>runtime|tooling)-shard-(?<index>[0-9]+)-of-(?<total>[1-9][0-9]*)$")
        | {scope, index: (.index | tonumber), total: (.total | tonumber)};
    def valid_scope($shards; $scope):
        [$shards[] | select(.scope == $scope)] as $group
        | ($group | length) as $count
        | $count > 0
          and all($group[];
              (.selector | selector_parts) as $selector
              | (.artifact_name | artifact_parts) as $artifact
              | $artifact.scope == $scope
                and $selector.index == $artifact.index
                and $selector.total == $artifact.total
                and $selector.total == $count
          )
          and ([$group[].selector | selector_parts | .index] | sort) == [range(0; $count)];

    . as $shards
    | ($shards | length) as $shard_count
    | type == "array"
      and $shard_count > 0
      and ([$shards[].scope] | unique) == ["runtime", "tooling"]
      and ([$shards[] | (.selector + ":" + .scope)] | unique | length) == $shard_count
      and ([$shards[].artifact_name] | unique | length) == $shard_count
      and valid_scope($shards; "runtime")
      and valid_scope($shards; "tooling")
' <<< "$shard_plan_json" >/dev/null; then
    echo "invalid mutation shard plan" >&2
    exit 1
fi

scratch_dir="$(mktemp -d "${TMPDIR:-/tmp}/ffhn-mutants-summary.XXXXXX")"
trap 'rm -rf -- "$scratch_dir"' EXIT
expected_artifacts="$scratch_dir/expected-artifacts.txt"
actual_artifacts="$scratch_dir/actual-artifacts.txt"
summary_rows="$scratch_dir/summary-rows.md"

jq -r '.[].artifact_name' <<< "$shard_plan_json" | LC_ALL=C sort > "$expected_artifacts"
if [[ -d "$artifact_root" ]]; then
    find "$artifact_root" -mindepth 1 -maxdepth 1 -type d -exec basename {} \; |
        LC_ALL=C sort > "$actual_artifacts"
else
    : > "$actual_artifacts"
fi

if ! cmp -s "$expected_artifacts" "$actual_artifacts"; then
    echo "mutation artifact identity set is incomplete or unexpected" >&2
    comm -23 "$expected_artifacts" "$actual_artifacts" |
        sed 's/^/  missing: /' >&2
    comm -13 "$expected_artifacts" "$actual_artifacts" |
        sed 's/^/  unexpected: /' >&2
    exit 1
fi

total=0
caught=0
missed=0
timed_out=0
unviable=0
artifacts_complete=true
: > "$summary_rows"

while IFS=$'\t' read -r scope selector artifact_name; do
    outcome_file="$artifact_root/$artifact_name/mutants.out/outcomes.json"
    if [[ ! -f "$outcome_file" ]]; then
        printf 'missing completed mutation outcome: %s\n' "$outcome_file" >&2
        artifacts_complete=false
        continue
    fi

    if ! jq -e '
        (.end_time | type == "string" and length > 0)
        and all(
            .total_mutants,
            .caught,
            .missed,
            .timeout,
            .unviable;
            type == "number" and . >= 0 and floor == .
        )
        and .total_mutants == (.caught + .missed + .timeout + .unviable)
    ' "$outcome_file" >/dev/null; then
        printf 'invalid or incomplete mutation outcome: %s\n' "$outcome_file" >&2
        artifacts_complete=false
        continue
    fi

    shard_total="$(jq -r '.total_mutants' "$outcome_file")"
    shard_caught="$(jq -r '.caught' "$outcome_file")"
    shard_missed="$(jq -r '.missed' "$outcome_file")"
    shard_timed_out="$(jq -r '.timeout' "$outcome_file")"
    shard_unviable="$(jq -r '.unviable' "$outcome_file")"
    total=$((total + shard_total))
    caught=$((caught + shard_caught))
    missed=$((missed + shard_missed))
    timed_out=$((timed_out + shard_timed_out))
    unviable=$((unviable + shard_unviable))
    # Markdown backticks are literal output, not shell expansion.
    # shellcheck disable=SC2016
    printf '| `%s` | `%s` | %s | %s | %s | %s | %s |\n' \
        "$scope" \
        "$selector" \
        "$shard_total" \
        "$shard_caught" \
        "$shard_missed" \
        "$shard_timed_out" \
        "$shard_unviable" >> "$summary_rows"
done < <(jq -r '.[] | [.scope, .selector, .artifact_name] | @tsv' <<< "$shard_plan_json")

if [[ "$artifacts_complete" != true ]]; then
    exit 1
fi

{
    echo "## cargo-mutants full-run summary"
    echo
    echo "| Scope | Shard | Mutants | Caught | Missed | Timed out | Unviable |"
    echo "| --- | --- | ---: | ---: | ---: | ---: | ---: |"
    cat "$summary_rows"
    echo
    echo "Total: $total mutants; $caught caught; $missed missed; $timed_out timed out; $unviable unviable."
} >> "$step_summary"
