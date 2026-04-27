use std::sync::Mutex;

// Locks without explicit "mark" — should match (p_lock matches, p_uncovered doesn't).
fn lock_only() {
    let m: Mutex<i32> = Mutex::new(0);
    let _g = m.lock();
    //~^ ops_lock_no_unlock
}

// Locks and then calls black_box on the guard (the "covered" marker).
// p_uncovered matches because both the lock call AND the black_box call are present.
// p_lock - p_uncovered subtracts this match → no diagnostic.
fn lock_and_mark() {
    let m: Mutex<i32> = Mutex::new(0);
    let g = m.lock();
    let _r = std::hint::black_box(g);
}

fn main() {
    lock_only();
    lock_and_mark();
}
