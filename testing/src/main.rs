
use std::mem;

mod ll {
    use libc::{c_char, c_int};

    extern "C" {
        pub fn instr(_: *const c_char) -> c_int;
    }
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
pub fn instr(s: &mut String) -> i32 {
    /* XXX: This is probably broken. */
    unsafe {
        let buf = s.as_bytes().as_ptr();
        let ret = ll::instr(mem::transmute(buf));

        // let capacity = s.capacity();
        // match s.find('\0') {
        //     Some(index) => s.as_mut_vec().set_len(index as usize),
        //     None => s.as_mut_vec().set_len(capacity),
        // }

        ret
    }
}

fn main() {
    let mut s = String::from("");
    let code = instr(&mut s);
}
