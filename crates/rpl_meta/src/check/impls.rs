use std::sync::Arc;

use parser::pairs;
use rustc_data_structures::fx::FxHashMap;

use super::CheckFnCtxt;
use crate::RPLMetaError;
use crate::context::MetaContext;
use crate::symbol_table::{AdtPats, FnInner, ImplInner, NonLocalMetaSymTab};
use crate::utils::Record;

pub(super) struct CheckImplCtxt<'i, 'r> {
    pub(super) meta_vars: Arc<NonLocalMetaSymTab<'i>>,
    pub(crate) adt_pats: &'r AdtPats<'i>,
    pub(super) impl_def: &'r mut ImplInner<'i>,
    pub(super) imports: &'r FxHashMap<&'i str, &'i pairs::Path<'i>>,
    pub(super) errors: &'r mut Vec<RPLMetaError<'i>>,
}

impl<'i> CheckImplCtxt<'i, '_> {
    pub(super) fn check_impl(&mut self, mctx: &MetaContext<'i>, rust_impl: &'i pairs::Impl<'i>) {
        let (_, _, _, _, _, _, _, _, fns, _) = rust_impl.get_matched();
        for rust_fn in fns.iter_matched() {
            // FIXME: check constraints
            let (rust_fn, _where_block) = rust_fn.get_matched();
            let (fn_name, mut fn_def) = FnInner::parse_from(mctx, rust_fn.FnSig().FnName(), None);
            let meta_vars = self.meta_vars.clone();
            CheckFnCtxt {
                meta_vars: meta_vars.clone(),
                adt_pats: self.adt_pats,
                impl_def: Some(self.impl_def),
                fn_def: &mut fn_def,
                imports: self.imports,
                errors: self.errors,
            }
            .check_fn(mctx, rust_fn);
            if let Some(ident) = fn_name {
                self.impl_def
                    .add_fn(mctx, &ident, (fn_def, meta_vars, self.adt_pats).into())
                    .or_record(self.errors);
            }
        }
    }
}
