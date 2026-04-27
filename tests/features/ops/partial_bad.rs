use std::sync::Mutex;

fn main() {
    let m: Mutex<i32> = Mutex::new(0);
    let _g = m.lock().unwrap();
    //~^ ops_partial_bad
}
