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
