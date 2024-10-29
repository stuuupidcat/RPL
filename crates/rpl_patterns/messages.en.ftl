rpl_patterns_offset_by_one = pointer out of bound
    .ptr_label = pointer created here
    .read_label = pointer read here
    .help = this is because `{$len_local}` exceeds the container's length by one
    .suggestion = did you mean this

rpl_patterns_unsound_slice_cast = it is unsound to cast any slice `&{$mutability}[{$ty}]` to a byte slice `&{$mutability}[u8]`
    .note = trying to cast from this value of `&{$mutability}[{$ty}]` type

rpl_patterns_use_after_drop = use a pointer from `{$ty}` after dropped
    .note = the `{$ty}` value is dropped here

rpl_patterns_reversed_parameters_when_constructing_a_vec = when using `Vec::from_raw_parts`, the `len` and `cap` parameters are reversed
    .note = the correct order is `Vec::from_raw_parts(ptr, len, cap)`