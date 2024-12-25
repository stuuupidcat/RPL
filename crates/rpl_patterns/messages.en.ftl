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

rpl_patterns_vec_set_len_to_extend = Use `Vec::set_len` to extend the length of a `Vec`, including uninitialized elements
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
