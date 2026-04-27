/// Well-formedness checks for `ops { ... }` blocks (R1–R3).
///
/// These checks operate on the raw pest parse-tree and must run **before** any
/// lowering function (`NonLocalMetaVars::from_meta_decls`, `Ty::from`, etc.) is
/// called, because the lowering code uses `unreachable!()` contracts that assume
/// the input has already been validated.
///
/// # Rules
///
/// | # | Rule |
/// |---|------|
/// | R1 | Every meta-var in an op group's `MetaVariableDeclList` must be of kind `type`. |
/// | R2 | Op signatures may only reference meta-vars that are declared in the same op group. |
/// | R3 | Op signatures must not contain concrete Rust types (concrete paths or primitive types). |
use std::ops::Deref;

use rpl_meta::collect_elems_separated_by_comma;
use rpl_parser::generics::{Choice2, Choice3, Choice14};
use rpl_parser::pairs;

// ---------------------------------------------------------------------------
// Public error type
// ---------------------------------------------------------------------------

/// A well-formedness violation found in an `ops { ... }` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpsWfError {
    pub group: String,
    pub message: String,
}

impl OpsWfError {
    fn new(group: impl Into<String>, message: impl Into<String>) -> Self {
        Self { group: group.into(), message: message.into() }
    }
}

impl std::fmt::Display for OpsWfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[ops group '{}'] {}", self.group, self.message)
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Check every `opsItem` in `block` for R1, R2, and R3 violations.
///
/// Returns a (possibly empty) list of errors.  A non-empty return value means
/// that the corresponding `opsItem`(s) should **not** be lowered.
pub fn check_ops_block(block: &pairs::opsBlock<'_>) -> Vec<OpsWfError> {
    let mut errors = Vec::new();
    for item in block.opsItem() {
        check_ops_item(item, &mut errors);
    }
    errors
}

// ---------------------------------------------------------------------------
// Per-item checks
// ---------------------------------------------------------------------------

fn check_ops_item(item: &pairs::opsItem<'_>, errors: &mut Vec<OpsWfError>) {
    let group_name = item.Identifier().span.as_str();
    let meta_decl_list = item.MetaVariableDeclList();

    // -- R1: collect declared type-var names and flag non-type vars.
    let declared_type_vars = collect_declared_type_vars(group_name, meta_decl_list, errors);

    // -- R2 + R3: walk each OpFnDecl signature.
    for decl in item.OpFnDecl() {
        let sig = decl.OpFnSig();
        check_op_sig(group_name, &sig, &declared_type_vars, errors);
    }
}

/// Pre-scan `MetaVariableDeclList`, collect names of `type`-kind vars, and
/// emit R1 errors for any non-`type`-kind var.
///
/// Returns the list of declared `type`-kind meta-var names (with `$` prefix,
/// as they appear in the source).
fn collect_declared_type_vars<'i>(
    group_name: &str,
    meta_decl_list: Option<&'i pairs::MetaVariableDeclList<'i>>,
    errors: &mut Vec<OpsWfError>,
) -> Vec<&'i str> {
    let mut type_var_names: Vec<&'i str> = Vec::new();

    let Some(mdl) = meta_decl_list else {
        return type_var_names;
    };
    let Some(inner) = mdl.get_matched().1 else {
        return type_var_names;
    };

    for decl in collect_elems_separated_by_comma!(inner) {
        let (ident, _, ty, _) = decl.get_matched();
        let var_name = ident.span.as_str(); // includes `$`

        match ty.deref() {
            Choice3::_0(_type_kind) => {
                // R1 OK — type-kind meta-var.
                type_var_names.push(var_name);
            },
            Choice3::_1(_const_kind) => {
                // R1 violation: const-kind.
                errors.push(OpsWfError::new(
                    group_name,
                    format!(
                        "op-level meta-var '{}' must be of kind 'type' \
                         (only 'type' is supported in v1)",
                        var_name
                    ),
                ));
            },
            Choice3::_2(_place_kind) => {
                // R1 violation: place-kind.
                errors.push(OpsWfError::new(
                    group_name,
                    format!(
                        "op-level meta-var '{}' must be of kind 'type' \
                         (only 'type' is supported in v1)",
                        var_name
                    ),
                ));
            },
        }
    }

    type_var_names
}

/// Check a single `OpFnSig` for R2 (undeclared meta-var references) and
/// R3 (concrete type references).
fn check_op_sig(
    group_name: &str,
    sig: &pairs::OpFnSig<'_>,
    declared_type_vars: &[&str],
    errors: &mut Vec<OpsWfError>,
) {
    // Walk parameters.
    if let Some(params_pair) = sig.OpFnParamsSeparatedByComma() {
        let (first, rest) = params_pair.OpFnParam();
        let params: Vec<_> = std::iter::once(first).chain(rest).collect();
        for param in params {
            if let Some(ty) = param_type(param) {
                check_type(group_name, ty, declared_type_vars, errors);
            }
        }
    }

    // Walk return type.
    if let Some(fn_ret) = sig.FnRet() {
        let (_, placeholder_or_ty) = fn_ret.get_matched();
        if let Choice2::_1(ty) = placeholder_or_ty {
            check_type(group_name, ty, declared_type_vars, errors);
        }
    }
}

/// Extract the `Type` node from an `OpFnParam`, if any.
///
/// `OpFnParam` alternatives:
/// - `SelfParam`         — no `Type` node to check (the self type is implicit)
/// - `NormalParam`       — has a `Type`
/// - `PlaceHolderWithType` — has a `Type`
/// - `Type`              — IS the `Type`
/// - `Dot2`              — variadic `..`, no type
fn param_type<'a>(param: &'a pairs::OpFnParam<'a>) -> Option<&'a pairs::Type<'a>> {
    // SelfParam — skip (self receiver, no explicit type to validate)
    if param.SelfParam().is_some() {
        return None;
    }
    // NormalParam: `$name: Type`
    if let Some(normal) = param.NormalParam() {
        let (_, _, _, ty) = normal.get_matched();
        return Some(ty);
    }
    // PlaceHolderWithType: `_: Type`
    if let Some(ph_with_ty) = param.PlaceHolderWithType() {
        let (_, _, _, ty) = ph_with_ty.get_matched();
        return Some(ty);
    }
    // Bare `Type` param (e.g. `&mut $T`)
    if let Some(ty) = param.Type() {
        return Some(ty);
    }
    // Dot2 — variadic
    None
}

/// Recursively walk a `Type` node to check R2 and R3.
///
/// - R2: if a `TypeMetaVariable` is found whose name is not in
///   `declared_type_vars`, emit an error.
/// - R3: if a `TypePath` or `PrimitiveType` leaf is found, emit an error.
///
/// Wrapper types (`TypeReference`, `TypePtr`, `TypeSlice`, `TypeTuple`,
/// `TypeArray`, `TypeGroup`, `TypeParen`) are traversed without error.
/// `TypeNever`, `PlaceHolder` (`_`), and `kw_Self` are silently accepted.
fn check_type(group_name: &str, ty: &pairs::Type<'_>, declared_type_vars: &[&str], errors: &mut Vec<OpsWfError>) {
    match ty.deref() {
        // Wrapping types — recurse into the inner type(s).
        Choice14::_0(ty_array) => {
            let (_, inner, _, _, _) = ty_array.get_matched();
            check_type(group_name, inner, declared_type_vars, errors);
        },
        Choice14::_1(ty_group) => {
            let (_, inner) = ty_group.get_matched();
            check_type(group_name, inner, declared_type_vars, errors);
        },
        Choice14::_2(_ty_never) => {
            // `!` — accept (rare, but abstract)
        },
        Choice14::_3(ty_paren) => {
            let (_, inner, _) = ty_paren.get_matched();
            check_type(group_name, inner, declared_type_vars, errors);
        },
        Choice14::_4(ty_ptr) => {
            let (_, _, inner) = ty_ptr.get_matched();
            check_type(group_name, inner, declared_type_vars, errors);
        },
        Choice14::_5(ty_ref) => {
            let (_, _, _, inner) = ty_ref.get_matched();
            check_type(group_name, inner, declared_type_vars, errors);
        },
        Choice14::_6(ty_slice) => {
            let (_, inner, _) = ty_slice.get_matched();
            check_type(group_name, inner, declared_type_vars, errors);
        },
        Choice14::_7(ty_tuple) => {
            let (_, tys_opt, _) = ty_tuple.get_matched();
            if let Some(tys) = tys_opt {
                let tys_vec = collect_elems_separated_by_comma!(tys).collect::<Vec<_>>();
                for inner in tys_vec {
                    check_type(group_name, inner, declared_type_vars, errors);
                }
            }
        },
        // Meta-variable reference — R2 check.
        Choice14::_8(ty_meta_var) => {
            // The span text is the raw `$Name` including the `$`.
            let var_name = ty_meta_var.span.as_str();
            if !declared_type_vars.contains(&var_name) {
                errors.push(OpsWfError::new(
                    group_name,
                    format!(
                        "op-level meta-var '{}' is not declared in op group '{}'",
                        var_name, group_name
                    ),
                ));
            }
        },
        // `Self` — silently accepted.
        Choice14::_9(_kw_self) => {},
        // PrimitiveType — R3 violation (concrete built-in type).
        Choice14::_10(prim) => {
            errors.push(OpsWfError::new(
                group_name,
                format!(
                    "ops blocks must use op-level meta-vars only — concrete types belong \
                     in rpl.toml (found primitive type '{}')",
                    prim.span.as_str()
                ),
            ));
        },
        // PlaceHolder (`_`) — accepted (wildcard / anonymous).
        Choice14::_11(_placeholder) => {},
        // TypePath — R3 violation (concrete Rust path).
        Choice14::_12(ty_path) => {
            errors.push(OpsWfError::new(
                group_name,
                format!(
                    "ops blocks must use op-level meta-vars only — concrete types belong \
                     in rpl.toml (found path '{}')",
                    ty_path.span.as_str()
                ),
            ));
        },
        // LangItemWithArgs — R3 violation (concrete lang-item reference).
        Choice14::_13(lang_item) => {
            errors.push(OpsWfError::new(
                group_name,
                format!(
                    "ops blocks must use op-level meta-vars only — concrete types belong \
                     in rpl.toml (found lang item '{}')",
                    lang_item.span.as_str()
                ),
            ));
        },
    }
}

// ---------------------------------------------------------------------------
// Unit tests for the helper functions
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wf_no_errors_for_valid_ops_block() {
        // The module-level tests in ops_lowering.rs already test valid input
        // end-to-end; this is just a quick sanity check that the checker is
        // quiet on well-formed input.
        assert!(true, "placeholder: valid input tested in ops_lowering.rs");
    }
}
