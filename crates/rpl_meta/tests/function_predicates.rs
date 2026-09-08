#![feature(rustc_private)]

use std::path::PathBuf;

use rpl_meta::arena::Arena;

#[test]
fn accepts_upstream_attribute_predicate_in_function_where_clause() {
    let arena = &*Box::leak(Box::new(Arena::default()));
    let sources = &*Box::leak(Box::new(vec![(
        PathBuf::from("/synthetic/function-predicates.rpl"),
        r#"
pattern function-predicates

patt {
    p = fn $f(..) -> _ {} where {
        has_attr($f, inline)
    }
}
"#
        .to_owned(),
    )]));
    let mut errors = Vec::new();
    rpl_meta::parse_and_collect(arena, sources, |error| errors.push(error.to_string()));
    assert!(errors.is_empty(), "unexpected predicate errors: {errors:#?}");
}
