#![feature(rustc_private)]

use std::path::Path;

use rpl_parser::{pairs, parse_main};

const ITEM_PATTERN: &str = r#"
pattern send-variance

patt {
    send_variance[
        $Wrapper: adt,
        $Parameter: type,
        $MappedType: type,
    ] = {
        struct $Wrapper<..> {
            ..
        }

        $marker:
        unsafe impl<..> Send for $Wrapper<..>
        where ..
        {}
    } where {
        true()
    }
}
"#;

fn parse(source: &str) -> pairs::main<'_> {
    parse_main(source, Path::new("/synthetic/item-pattern.rpl")).expect("item pattern should parse")
}

#[test]
fn parses_non_exhaustive_struct_and_bound_impl() {
    let main = parse(ITEM_PATTERN);
    let pattern = main.RPLPattern();
    let patt = pattern
        .Block()
        .into_iter()
        .find_map(|block| block.pattBlock())
        .expect("patt block");
    let item = patt.RPLPatternItem().into_iter().next().expect("pattern item");

    let meta_decls = item.MetaVariableDeclList().expect("meta variable declarations");
    let adt_decl = meta_decls
        .MetaVariableDeclsSeparatedByComma()
        .expect("non-empty meta variable declarations")
        .MetaVariableDecl()
        .0
        .MetaVariableType();
    assert!(adt_decl.kw_adt().is_some());
    let bundle = item
        .RustItemsOrPatternOperation()
        .RustItemsWithConstraint()
        .expect("item bundle");
    assert!(
        bundle.WhereBlock().is_some(),
        "bundle-level predicates should be retained"
    );
    let items = bundle.RustItemWithConstraint();
    assert_eq!(items.len(), 2);

    let struct_pattern = items[0].RustItem().Struct().expect("struct pattern");
    assert_eq!(struct_pattern.MetaVariable().span.as_str(), "$Wrapper");
    assert!(struct_pattern.ItemGenericWildcard().is_some());
    assert!(
        struct_pattern
            .StructFields()
            .expect("struct fields")
            .NonExhaustiveFields()
            .is_some()
    );

    let impl_pattern = items[1].RustItem().Impl().expect("impl pattern");
    assert_eq!(
        impl_pattern
            .ItemBinding()
            .expect("impl binding")
            .MetaVariable()
            .span
            .as_str(),
        "$marker"
    );
    assert!(impl_pattern.kw_unsafe().is_some());
    assert!(impl_pattern.ItemGenericWildcard().is_some());
    assert_eq!(
        impl_pattern.ImplKind().expect("trait impl").Path().span.as_str().trim(),
        "Send"
    );
    let self_ty = impl_pattern
        .ImplSelfType()
        .ItemAdtType()
        .expect("non-exhaustive ADT self type");
    assert_eq!(self_ty.MetaVariable().span.as_str(), "$Wrapper");
    let _ = self_ty.ItemGenericWildcard();
    assert!(impl_pattern.ImplWhereClause().is_some());
}

#[test]
fn preserves_legacy_struct_and_impl_syntax() {
    let main = parse(
        r#"
pattern legacy-items
patt {
    p = {
        struct $Wrapper {}
        unsafe impl Send for $Wrapper {}
    }
}
"#,
    );
    let item = main
        .RPLPattern()
        .Block()
        .into_iter()
        .find_map(|block| block.pattBlock())
        .expect("patt block")
        .RPLPatternItem()[0]
        .RustItemsOrPatternOperation()
        .RustItemsWithConstraint()
        .expect("item bundle")
        .RustItemWithConstraint()[1]
        .RustItem()
        .Impl()
        .expect("legacy impl");
    assert!(
        item.ImplSelfType().Type().is_some(),
        "legacy self types should use the ordinary Type arm"
    );
}

#[test]
fn item_wildcard_does_not_leak_into_function_types() {
    let result = parse_main(
        r#"
pattern misplaced-item-wildcard
patt {
    p = fn _ ($value: Vec<..>) {}
}
"#,
        Path::new("/synthetic/misplaced-item-wildcard.rpl"),
    );
    assert!(result.is_err());
}

#[test]
fn impl_where_wildcard_requires_dotdot() {
    let result = parse_main(
        r#"
pattern malformed-impl-where
patt {
    p = {
        struct $Wrapper<..> { .. }
        $marker: unsafe impl<..> Send for $Wrapper<..> where {} {}
    }
}
"#,
        Path::new("/synthetic/malformed-impl-where.rpl"),
    );
    assert!(result.is_err());
}

#[test]
fn parses_named_fields_before_struct_rest() {
    let main = parse(
        r#"
pattern named-field-rest
patt {
    p[$Wrapper: adt, $T: type] = {
        struct $Wrapper<..> {
            $field: $T,
            ..
        }
    }
}
"#,
    );
    let fields = main
        .RPLPattern()
        .Block()
        .into_iter()
        .find_map(|block| block.pattBlock())
        .expect("patt block")
        .RPLPatternItem()[0]
        .RustItemsOrPatternOperation()
        .RustItemsWithConstraint()
        .expect("item bundle")
        .RustItemWithConstraint()[0]
        .RustItem()
        .Struct()
        .expect("struct")
        .StructFields()
        .expect("fields")
        .NonExhaustiveFields()
        .expect("field rest");
    assert_eq!(fields.Field().len(), 1);
}

#[test]
fn adt_remains_an_ordinary_identifier_outside_meta_variable_types() {
    parse(
        r#"
pattern contextual-adt
patt {
    p = fn adt (..) {}
}
"#,
    );
}

#[test]
fn parses_access_metavariable_type() {
    let main = parse(
        r#"
pattern access-witness
patt {
    p[$Access: access] = {
        struct $Wrapper<..> { .. }
    } where {
        true()
    }
}
"#,
    );
    let access_ty = main
        .RPLPattern()
        .Block()
        .into_iter()
        .find_map(|block| block.pattBlock())
        .expect("patt block")
        .RPLPatternItem()[0]
        .MetaVariableDeclList()
        .expect("meta variable declarations")
        .MetaVariableDeclsSeparatedByComma()
        .expect("non-empty meta variable declarations")
        .MetaVariableDecl()
        .0
        .MetaVariableType();
    assert!(access_ty.kw_access().is_some());
}

#[test]
fn bundle_guard_is_unambiguous_and_cannot_be_repeated() {
    let result = parse_main(
        r#"
pattern duplicate-bundle-guard
patt {
    p = {
        struct $Wrapper<..> { .. }
    } where {
        true()
    } where {
        true()
    }
}
"#,
        Path::new("/synthetic/duplicate-bundle-guard.rpl"),
    );
    assert!(result.is_err());
}

#[test]
fn negative_impl_patterns_are_not_accepted_yet() {
    let result = parse_main(
        r#"
pattern negative-impl
patt {
    p = {
        struct $Wrapper<..> { .. }
        unsafe impl<..> !Send for $Wrapper<..> {}
    }
}
"#,
        Path::new("/synthetic/negative-impl.rpl"),
    );
    assert!(result.is_err());
}
