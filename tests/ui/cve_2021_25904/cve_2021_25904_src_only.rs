//@compile-flags: -Z inline-mir=false
use std::slice;

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn copy_from_raw_parts<I, IU>(mut src: I, mut src_linesize: IU)
//~^ERROR: it is unsound to trust pointers from passed-in iterators in a public safe function
where
    I: Iterator<Item = *const u8>,
    IU: Iterator<Item = usize>,
{
    let rr = src.next().unwrap();
    let s_linesize = src_linesize.next().unwrap();
    let ss = unsafe { slice::from_raw_parts(rr, s_linesize) };
}

fn main() {
    panic!("Todo.");
}
