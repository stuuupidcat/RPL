rpl_patterns_offset_by_one = pointer out of bound
    .ptr_label = pointer created here
    .read_label = pointer read here
    .help = this is because `{$len_local}` exceeds the container's length by one
    .suggestion = did you mean this

rpl_patterns_unsound_slice_cast = it is unsound to cast any slice `&{$mutability}[{$ty}]` to a byte slice `&{$mutability}[u8]`
    .note = trying to cast from this value of `&{$mutability}[{$ty}]` type

rpl_patterns_use_after_drop = use a pointer from `{$ty}` after dropped
    .note = the `{$ty}` value is dropped here

rpl_patterns_misordered_parameters = misordered parameters `len` and `cap` in `Vec::from_raw_parts`
    .help = the correct order is `Vec::from_raw_parts(ptr, len, cap)`

rpl_patterns_wrong_assumption_of_fat_pointer_layout = wrong assumption of fat pointer layout
    .ptr_transmute_label = ptr transmute here 
    .get_data_ptr_label = try to get data ptr from first 8 bytes here
    .help = the Rust Compiler does not expose the layout of fat pointers

rpl_patterns_rust_str_as_c_str = it is usually a bug to cast a `&str` to a `*const libc::c_char`, and then pass it to an extern function
    .label = the string is here
    .note  = the `*const libc::c_char` is created here
    .help  = try `std::ffi::CStr` instead

rpl_patterns_lengthless_buffer_passed_to_extern_function = it is usually a bug to pass a buffer pointer to an extern function without specifying its length
    .label = the pointer is passed here

rpl_patterns_wrong_assumption_of_layout_compatibility = wrong assumption of layout compatibility from `{$type_from}` to `{$type_to}`
    .note  = casted from this
    .help  = it's not guaranteed by Rust standard library. See https://github.com/rust-lang/rust/pull/78802

rpl_patterns_vec_set_len_to_extend = Use `Vec::set_len` to extend the length of a `Vec`, potentially including uninitialized elements
    .label = `Vec` created here
    .note = make sure all elements are initialized before using them

rpl_patterns_vec_set_len_to_truncate = Use `Vec::set_len` to truncate the length of a `Vec`
    .help = Consider using `Vec::truncate` instead

rpl_patterns_trust_exact_size_iterator = it is unsound to trust return value of `std::iter::ExactSizeIterator::len` and pass it to an unsafe function like `std::vec::Vec::set_len`, which may leak uninitialized memory
    .len_label = `std::iter::ExactSizeIterator::len` used here
    .help = incorrect implementation of `std::iter::ExactSizeIterator::len` must not cause safety issues

rpl_patterns_slice_from_raw_parts_uninitialized = it violates the precondition of `{$fn_name}` to create a slice from uninitialized data
    .slice_label = slice created here
    .vec_label   = `std::vec::Vec` created but not initialized
    .len_label   = slice created with this length
    .ptr_label   = slice created with this pointer
    .help        = See https://doc.rust-lang.org/std/slice/fn.{$fn_name}.html

rpl_patterns_set_len_uninitialized = it violates the precondition of `Vec::set_len` to extend a `Vec`'s length without initializing its content in advance
    .vec_label = `Vec` created here
    .help = before calling `set_len` to extend its length, make sure all elements are initialized, using such as `spare_capacity_mut` or `as_mut_ptr`

rpl_patterns_get_mut_in_rc_unsafecell = Obtaining a mutable reference to the value wrapped by `Rc<UnsafeCell<$T>>` is unsound
    .note = there will be multiple mutable references to the value at the same time
    .help = use `std::cell::RefCell` instead

rpl_patterns_drop_uninit_value = Possibly dropping an uninitialized value

rpl_patterns_from_raw_parts = it is unsound to trust pointers from passed-in iterators in a public safe function
    .ptr_label = pointer created here
    .slice_label = used here to create a slice from the pointer
    .help = please mark the function as unsafe

rpl_patterns_unsound_cast_between_u64_and_atomic_u64 = it is unsound to cast between `u64` and `AtomicU64`
    .note = the alignment of `u64` is smaller than `AtomicU64` on many 32-bits platforms
    .src_label = u64 created here

rpl_patterns_thread_local_static_ref = it is unsound to expose a `&'static {$ty}` from a thread-local where `{$ty}` is `Sync`
    .sync_help = `{$ty}` is `Sync` so that it can shared among threads
    .help = the thread local is destroyed after the thread has been destroyed, and the exposed `&'static {$ty}` may outlive the thread it is exposed to
    .label = thread local used here
    .ret_label = `&'static {$ty}` returned here

rpl_patterns_deref_null_pointer = Dereference of a possibly null pointer
    .ptr_label = pointer created here
    .deref_label = dereference here
    .note = this is because the pointer may be null