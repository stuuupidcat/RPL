use rustc_session::config::OptLevel;

pub(crate) fn inline_mir(sess: &rustc_session::Session) -> bool {
    // FIXME(#127234): Coverage instrumentation currently doesn't handle inlined
    // MIR correctly when Modified Condition/Decision Coverage is enabled.
    if sess.instrument_coverage_mcdc() {
        return false;
    }

    if let Some(enabled) = sess.opts.unstable_opts.inline_mir {
        return enabled;
    }

    match sess.mir_opt_level() {
        0 | 1 => false,
        2 => {
            (sess.opts.optimize == OptLevel::Default || sess.opts.optimize == OptLevel::Aggressive)
                && sess.opts.incremental == None
        },
        _ => true,
    }
}
