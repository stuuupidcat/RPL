rpl_patterns_offset_by_one = pointer out of bound
    .ptr_label = pointer created here
    .read_label = pointer read here
    .help = this is because `{$len_local}` exceeds the container's length by one
    .suggestion = did you mean this

rpl_patterns_unsound_slice_cast = it is unsound to cast any slice `&{$mutability}[{$ty}]` to a byte slice `&{$mutability}[u8]`
    .cast_to_label = casted to a byte slice here
    .note = trying to cast from this value of `&{$mutability}[{$ty}]` type

rpl_patterns_use_after_drop = use a pointer from `{$ty}` after dropped
    .use_label = used here
    .note = the `{$ty}` value is dropped here

rpl_patterns_misordered_parameters = misordered parameters `len` and `cap` in `Vec::from_raw_parts`
    .label = `Vec::from_raw_parts` called here
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
    .cast_to_label = casted to `{$type_to}` here
    .note  = casted from this
    .help  = it's not guaranteed by Rust standard library. See https://github.com/rust-lang/rust/pull/78802

rpl_patterns_vec_set_len_to_extend = Use `Vec::set_len` to extend the length of a `Vec`, potentially including uninitialized elements
    .set_len_label = `Vec::set_len` called here
    .vec_label = `Vec` created here
    .note = make sure all elements are initialized before using them

rpl_patterns_vec_set_len_to_truncate = Use `Vec::set_len` to truncate the length of a `Vec`
    .set_len_label = `Vec::set_len` called here
    .help = Consider using `Vec::truncate` instead

rpl_patterns_trust_exact_size_iterator = it is unsound to trust return value of `std::iter::ExactSizeIterator::len` and pass it to an unsafe function like `std::vec::Vec::set_len`, which may leak uninitialized memory
    .label = length used here in `{$fn_name}`
    .note = `std::iter::ExactSizeIterator::len` may not be implemented correctly, and it should be used as a hint rather than a fact
    .len_label = `std::iter::ExactSizeIterator::len` used here
    .help = incorrect implementation of `std::iter::ExactSizeIterator::len` must not cause safety issues, and consider using `std::iter::TrustedLen` instead if it's stabilized

rpl_patterns_slice_from_raw_parts_uninitialized = it violates the precondition of `{$fn_name}` to create a slice from uninitialized data
    .slice_label = slice created here
    .vec_label   = `std::vec::Vec` created but not initialized
    .len_label   = slice created with this length
    .ptr_label   = slice created with this pointer
    .help        = See https://doc.rust-lang.org/std/slice/fn.{$fn_name}.html

rpl_patterns_set_len_uninitialized = it violates the precondition of `Vec::set_len` to extend a `Vec`'s length without initializing its content in advance
    .set_len_label = `Vec::set_len` called here
    .vec_label = `Vec` created here
    .help = before calling `set_len` to extend its length, make sure all elements are initialized, using such as `spare_capacity_mut` or `as_mut_ptr`

rpl_patterns_get_mut_in_rc_unsafecell = Obtaining a mutable reference to the value wrapped by `Rc<UnsafeCell<$T>>` may be unsound
    .get_mut_label = `UnsafeCell::get_mut` called here
    .note = there will be multiple mutable references to the value at the same time
    .help = use `std::cell::RefCell` instead

rpl_patterns_drop_uninit_value = Possibly dropping an uninitialized value
    .drop_label = dropped here
    .help = assigning to a dereferenced pointer will cause previous value to be dropped, and try using `ptr::write` instead

rpl_patterns_unvalidated_slice_from_raw_parts = it is unsound to trust pointers from passed-in iterators in a public safe function
    .src_label = source iterator found here
    .ptr_label = pointer created here
    .slice_label = used here to create a slice from the pointer
    .help = consider marking the function as unsafe

rpl_patterns_unsound_cast_between_u64_and_atomic_u64 = it is unsound to cast between `u64` and `AtomicU64`
    .cast_label = casted here
    .note = the alignment of `u64` is smaller than `AtomicU64` on many 32-bits platforms
    .src_label = u64 created here

rpl_patterns_thread_local_static_ref = it is unsound to expose a `&'static {$ty}` from a thread-local where `{$ty}` is `Sync`
    .sync_help = `{$ty}` is `Sync` so that it can shared among threads
    .help = the thread local is destroyed after the thread has been destroyed, and the exposed `&'static {$ty}` may outlive the thread it is exposed to
    .fn_label = function returning `&'static {$ty}` here
    .thread_local_label = thread local used here
    .ret_label = `&'static {$ty}` returned here

rpl_patterns_deref_null_pointer = Dereference of a possibly null pointer
    .ptr_label = pointer created here
    .deref_label = dereference here
    .note = this is because the pointer may be null

rpl_patterns_deref_unchecked_ptr_offset = it is unsound to dereference a pointer that is offset using an unchecked integer
    .reference_label = dereferenced here
    .ptr_label = pointer created here
    .offset_label = offset passed in here
    .help = check whether it's in bound before dereferencing

rpl_patterns_unsound_pin_project = it is unsound to call `Pin::new_unchecked` on a mutable reference that can be freely moved
    .pin_label = `Pin::new_unchecked` called here
    .ref_label = mutable reference passed into a public function here
    .note = type `{$ty}` doesn't implement `Unpin`

rpl_patterns_unchecked_ptr_offset = it is an undefined behavior to offset a pointer using an unchecked integer
    .offset_label = offset here
    .ptr_label = pointer used here
    .help = check whether it's in bound before offsetting
    .note = See the safety section in https://doc.rust-lang.org/std/primitive.pointer.html#method.offset

rpl_patterns_cassandra_iter_next_ptr_passed_to_cass_iter_get = it will be an undefined behavior to pass a pointer returned by `cass_iterator_next` to `cass_iterator_get_*` in a `std::iter::Iterator` implementation
    .cass_iter_next_label = `cass_iterator_next` called here
    .note = `cass_iterator_next` will invalidate the current item when called
    .help = consider implementing a `LendingIterator` instead

rpl_patterns_slice_from_raw_parts_uninitialized_ = it violates the precondition of `{$fn_name}` to create a slice from uninitialized data
    .slice_label = slice created here
    .len_label   = slice created with this length
    .ptr_label   = slice created with this pointer
    .help        = See https://doc.rust-lang.org/std/slice/fn.{$fn_name}.html

rpl_patterns_private_function_marked_inline =  it usually isn’t necessary to apply #[inline] to private functions
    .label = `#[inline]` applied here
    .note = within a crate, the compiler generally makes good inline decisions
    .help = See https://matklad.github.io/2021/07/09/inline-in-rust.html
