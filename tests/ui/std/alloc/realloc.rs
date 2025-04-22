//@ignore-on-host
use std::alloc::{Layout, alloc, dealloc, realloc};

fn use_after_realloc() {
    let layout = Layout::array::<u8>(16).unwrap();
    unsafe {
        let ptr = alloc(layout) as *mut u8;
        assert!(!ptr.is_null());
        ptr.write(42);
        let new_ptr = realloc(ptr as *mut u8, layout, layout.size() * 2) as *mut u8;
        assert!(!new_ptr.is_null());
        // use the old pointer after realloc
        let x = *ptr;
        assert_eq!(x, 42);
        dealloc(ptr as *mut u8, layout);
    }
}
