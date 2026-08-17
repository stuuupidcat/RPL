# Rudra Sync variance fixtures

These UI tests adapt every `Sync`-bearing fixture from Rudra commit
`e12e6d3b370efaf19479815f82aa935ead54bae9` under `tests/send_sync/`.

RPL intentionally does not preserve Rudra's naive missing-`Sync` reports. The
current resource-access backend reports `wild_channel`, whose safe `&self` API
returns `&Q`, and passes the no-API, phantom-only, and acknowledged false-positive
fixtures. The focused `sync_variance_*` tests cover the independent `Send` and
`Sync` access requirements that are absent from Rudra's pinned corpus.
