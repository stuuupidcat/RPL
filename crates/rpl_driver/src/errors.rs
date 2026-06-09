use rustc_macros::Diagnostic;
use rustc_session::declare_tool_lint;
use rustc_span::Symbol;

declare_tool_lint!(
    pub rpl_interface::TIMING,
    Warn,
    "timing information for RPL interface"
);

#[derive(Diagnostic)]
#[diag("{$time} ns has been used during {$stage} after checking {$crate_name}")]
pub struct Timing {
    /// Used time in nanoseconds
    pub time: u64,
    /// The stage of the checking process
    pub stage: &'static str,
    /// The name of the crate being processed
    pub crate_name: Symbol,
}
