//@ignore-on-host
//@compile-flags: -Z inline-mir=false
use std::slice;

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn copy_from_raw_parts<I, IU>(mut src: I, mut src_linesize: IU)
where
    I: Iterator<Item = *const u8>,
    IU: Iterator<Item = usize>,
{
    for rr in src {
        let s_linesize = src_linesize.next().unwrap();
        let ss = unsafe { slice::from_raw_parts(rr, s_linesize) };
    }
}

fn main() {
    panic!("Todo.");
}
