//@ignore-on-host
//@compile-flags: -Z inline-mir=false
use std::slice;

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn copy_from_raw_parts<I, IU>(mut src: I, mut src_linesize: IU)
where
    I: Iterator<Item = *const u8>,
    IU: Iterator<Item = usize>,
{
    for (rr, s_linesize) in src.zip(src_linesize) {
        let ss = unsafe { slice::from_raw_parts(rr, s_linesize) };
        // ...
    }
}

fn main() {
    panic!("Todo.");
}
