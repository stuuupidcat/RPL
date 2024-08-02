// use rpl_middle::ty;
// use rpl_query_impl::QueryCtxt;
// use rpl_query_system::query::print_query_stack;
// use rustc_errors::DiagCtxt;

/*
pub fn try_print_query_stack(dcx: &DiagCtxt, num_frames: Option<usize>, file: Option<std::fs::File>) {
    eprintln!("\nrpl query stack during panic:");

    // Be careful relying on global state here: this code is called from
    // a panic hook, which means that the global `DiagCtxt` may be in a weird
    // state if it was responsible for triggering the panic.
    let i = ty::tls::with_context_opt(|icx| {
        if let Some(icx) = icx {
            ty::print::with_no_queries!(print_query_stack(
                QueryCtxt::new(icx.bcx),
                icx.query,
                dcx,
                num_frames,
                file,
            ))
        } else {
            0
        }
    });

    if num_frames.is_none() || num_frames >= Some(i) {
        eprintln!("end of rpl query stack");
    } else {
        eprintln!("we're just showing a limited slice of the query stack");
    }
}
*/
