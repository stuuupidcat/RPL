# Rudra Send/Sync variance fixtures

These UI tests adapt all 12 fixtures from Rudra commit
`e12e6d3b370efaf19479815f82aa935ead54bae9` under `tests/send_sync/`. Ten contain
an unsafe `Sync` impl and use `sync_variance.rpl`; the Send-only
`okay_ptr_like` and `wild_send` cases use `send_variance.rpl`.

RPL intentionally does not preserve Rudra's naive missing-`Sync` reports. The
current resource-access backend reports `wild_channel`, whose safe `&self` API
returns `&Q`, and passes the no-API, phantom-only, and acknowledged false-positive
fixtures. The focused `sync_variance_*` tests cover the independent `Send` and
`Sync` access requirements that are absent from Rudra's pinned corpus.

RPL also intentionally reports both impls in `okay_ptr_like`: its wrappers own
`P`, and `P: Sync` does not imply `P: Send`. Rudra classifies that fixture as
okay because its pointer-like heuristic accepts the weaker bound; this is a
known semantic difference rather than a missing corpus case.

The pattern surface correlates each resource and trait-support query with an
explicit access witness. This preserves method and impl bounds and leaves room
for an exact evaluator. The first evaluator is deliberately signature-based: it
recognizes direct safe `&self` methods and can still miss body-mediated, trait,
guard, alias, and specialized access, or over-report by-value signatures whose
bodies never transfer the value across threads.
