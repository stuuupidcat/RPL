//@ ignore-on-host

struct Session<'a> {
    s: i32,
    _f: &'a (),
}

fn str_to_cstring(s: &str) -> Result<std::ffi::CString, Error> {
    std::ffi::CString::new(s).map_err(|_| Error)
}

struct Error;

macro_rules! check {
    ($expr:expr) => {{
        let ret = $expr;
        if ret != 0 {
            return Err(Error);
        }
    }};
}

impl Session<'_> {
    pub fn attach(&mut self, table: Option<&str>) -> Result<(), Error> {
        let table = if let Some(table) = table {
            str_to_cstring(table)?.as_ptr()
        } else {
            //~^ NOTE: the `std::ffi::CString` value is dropped here
            std::ptr::null()
        };
        unsafe { check!(ffi::sqlite3session_attach(self.s, table)) };
        //~^ ERROR: use a pointer from `std::ffi::CString` after dropped
        Ok(())
    }
}

mod ffi {
    extern "C" {
        pub fn sqlite3session_attach(s: i32, table: *const std::ffi::c_char) -> i32;
    }
}
