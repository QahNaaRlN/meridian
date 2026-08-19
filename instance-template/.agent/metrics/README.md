# Gate-run metrics

`validate-log.jsonl` (created on first real-Instance run of
`node scripts/validate-and-log.mjs`, not committed empty) is an append-only
JSON-lines log, one record per gate run against this Instance. Schema and
collection rules: `$MERIDIAN_KERNEL/standards/workspace/feedback-and-metrics.md`.

No maintained dashboard or rollup script exists yet — build one only when
the log is large enough that eyeballing stops being enough.
