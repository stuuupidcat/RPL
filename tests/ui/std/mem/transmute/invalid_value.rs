use std::mem::transmute;

// produce a value with an invalid state
pub fn invalid_value() -> bool {
    let x: u8 = 10;
    unsafe { transmute::<u8, bool>(x) } //~ERROR: it is unsound to transmute a type to a boolean
}

fn main() {}
