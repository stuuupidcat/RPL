clear

set -euxo pipefail


export RUSTC_ICE=0
export RUSTC_LOG_COLOR=always
export RUSTC_LOG="rpl=trace"
export RUST_BACKTRACE=1

# RPL_PATS="tests/ui/features/diff_pat/default.rpl" cargo +rpl-dbg uitest -- "tests/ui/features/diff_pat" 2>&1 | tee .ansi
# RPL_PATS="tests/ui/features/diff_pat/meta_var.rpl" cargo +rpl-dbg uitest -- "tests/ui/features/diff_pat" 2>&1 | tee .ansi
# RPL_PATS="tests/ui/features/diff_pat/label.rpl" cargo +rpl-dbg uitest -- "tests/ui/features/diff_pat" 2>&1 | tee .ansi
RPL_PATS="tests/ui/features/diff_pat/label.rpl" cargo +rpl-dbg run --bin rpl-driver -- "tests/ui/features/diff_pat/test.rs" 2>&1 | tee .ansi
# RPL_PATS="tests/ui/features/diff_pat" cargo +rpl-dbg uitest -- "tests/ui/features/diff_pat" 2>&1 | tee .ansi

# Task 13: lock_unlock end-to-end ops test
# Exercises the abstract-ops feature: parser, lowering, OpRef IR, OpsConfig
# substitution, cartesian iteration, and matcher OpRef resolution against
# std::sync::Mutex.  The workspace rpl.toml provides the [[ops.sync]] instance.
RPL_PATS="tests/features/ops/lock_unlock.rpl" \
  cargo run --bin rpl-driver -- "tests/features/ops/lock_unlock.rs" 2>&1 | tee .ansi
