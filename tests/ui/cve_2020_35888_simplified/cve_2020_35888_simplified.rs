use std::alloc::{alloc, Layout};

struct DropDetector(u32);

impl Drop for DropDetector {
    fn drop(&mut self) {
        //println!("Dropping value: {} at {:?}", self.0, self as *const _);
    }
}

// #[rpl::dump_mir(dump_cfg, dump_ddg)]
fn main() {
    let layout = Layout::new::<DropDetector>();

    let ptr: *mut DropDetector = unsafe { alloc(layout) as *mut DropDetector };

    unsafe {
        (*ptr) = DropDetector(12345);
        //~^ ERROR: Possibly dropping an uninitialized value
        std::ptr::drop_in_place(ptr);
    }
}
