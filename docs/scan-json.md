# Scan JSON Output

`eltk scan --json` emits a machine-readable summary of Claude Code usage.
The output is intended for fixture comparison and downstream tools that need
stable token totals without parsing the text report.

```sh
cargo run -p eltk -- scan --root /path/to/claude --json
```

Add `--include-excluded` when displayed totals should include synthetic and
API-error rows.

## Top-Level Shape

The current format version is `eltk_scan_v1`.

```json
{
  "format": "eltk_scan_v1",
  "agent": "claude_code",
  "include_excluded": false,
  "total_scope": "included records",
  "stats": {},
  "totals": {},
  "source_errors": []
}
```

| Field | Meaning |
| --- | --- |
| `format` | JSON contract identifier. Consumers should check this before parsing. |
| `agent` | Source adapter that produced the scan. Currently `claude_code`. |
| `include_excluded` | Whether displayed totals include synthetic/API-error rows. |
| `total_scope` | Human-readable description of what `totals.displayed` contains. |
| `stats` | Source and record counters for the scan. |
| `totals` | Token and server-tool totals by scope. |
| `source_errors` | Source-level scan errors that did not stop the whole scan. |

## Stats

`stats` contains unsigned integer counters.

| Field | Meaning |
| --- | --- |
| `sources_discovered` | JSONL sources found under the configured roots. |
| `sources_scanned` | Sources attempted during the scan. |
| `records_seen` | JSONL lines read from scanned sources. |
| `records_emitted` | Usage records accepted before merge/deduplication. |
| `records_after_merge` | Usage records remaining after merge/deduplication. |
| `warnings` | Parser warnings plus source-level scan errors. |
| `excluded_records` | Synthetic plus API-error rows with usage. |
| `synthetic_records` | Excluded rows where the model is `<synthetic>`. |
| `api_error_records` | Excluded rows marked as API error messages. |

## Totals

`totals` contains six scopes:

| Scope | Meaning |
| --- | --- |
| `displayed` | Totals shown as the main result for the selected mode. |
| `included` | Accepted usage records after merge/deduplication. |
| `excluded` | Synthetic plus API-error rows. |
| `synthetic` | Synthetic rows only. |
| `api_error` | API-error rows only. |
| `observed` | Included plus excluded usage. |

By default, `displayed` equals `included`. With `--include-excluded`,
`displayed` equals `observed`.

Each total scope has the same fields:

| Field | Meaning |
| --- | --- |
| `input_tokens` | Input token bucket reported by the source usage object. |
| `output_tokens` | Output token bucket reported by the source usage object. |
| `cache_read_input_tokens` | Input tokens read from cache. |
| `cache_creation_input_tokens` | Input tokens written to cache. |
| `reasoning_output_tokens` | Reasoning tokens reported separately from output tokens. |
| `extra_reported_tokens` | Positive difference between the source-reported total and the computed token total (`bucket_tokens + reasoning_output_tokens`). |
| `bucket_tokens` | `input + output + cache_read + cache_creation`. |
| `total_tokens` | `bucket_tokens + reasoning_output_tokens + extra_reported_tokens`. |
| `server_tool_requests` | Server-side tool request count. This is not a token count. |

Cache creation TTL fields are folded into `cache_creation_input_tokens` before
totals are emitted. If TTL-specific cache creation fields disagree with the
flat cache creation field, the TTL sum is used.

## Excluded Usage

Synthetic and API-error rows are collected because they can matter for local
auditing, but they are excluded from displayed totals by default. The separate
`synthetic`, `api_error`, and `excluded` scopes let consumers choose whether to
count those rows for their own reporting.

Use `observed` when every local token bucket should be counted. Use `included`
when only accepted assistant usage records should be counted. Use `displayed`
when following the CLI mode selected by the user.

## Source Errors

`source_errors` is an array of objects:

```json
[
  {
    "source": "/path/to/source.jsonl",
    "message": "failed source"
  }
]
```

The `source` value is rendered as a lossy string so non-UTF-8 paths do not
prevent JSON output.
