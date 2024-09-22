use crate::symbol_table::CheckError;
use crate::SymbolTable;
use proc_macro2::Ident;
use rpl_mir_syntax::*;

pub(crate) fn check_mir(mir: &Mir) -> syn::Result<()> {
    CheckCtxt::new().check_mir(mir)
}

#[derive(Default)]
struct CheckCtxt {
    symbols: SymbolTable,
}

impl CheckCtxt {
    fn new() -> Self {
        Self::default()
    }
    fn check_mir(&mut self, mir: &Mir) -> syn::Result<()> {
        for meta in &mir.metas {
            self.check_meta(meta)?;
        }
        for decl in &mir.declarations {
            self.check_decl(decl)?;
        }
        for stmt in &mir.statements {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }
    fn check_meta(&mut self, meta: &Meta) -> syn::Result<()> {
        meta.meta
            .content
            .iter()
            .map(|item| self.check_meta_item(item))
            .collect()
    }
    fn check_meta_item(&mut self, meta_item: &MetaItem) -> syn::Result<()> {
        match meta_item.kind {
            MetaKind::Ty(_) => self.symbols.add_ty_var(meta_item.clone())?,
        }
        Ok(())
    }

    fn check_decl(&mut self, decl: &Declaration) -> syn::Result<()> {
        match decl {
            Declaration::TypeDecl(TypeDecl { ty, ident, .. }) => self.symbols.add_type(ident.clone(), ty.clone()),
            Declaration::UsePath(UsePath { path, .. }) => self.symbols.add_path(path.clone()),
            Declaration::LocalDecl(LocalDecl { ident, ty, init, .. }) => {
                self.symbols.add_local(ident.clone(), ty.clone());
                if let Some(LocalInit { rvalue_or_call, .. }) = init {
                    self.check_rvalue_or_call(rvalue_or_call)?;
                }
                Ok(())
            },
        }
    }

    fn check_stmt(&mut self, stmt: &Statement) -> syn::Result<()> {
        match stmt {
            Statement::Assign(
                Assign {
                    place, rvalue_or_call, ..
                },
                _,
            ) => {
                self.check_place(place)?;
                self.check_rvalue_or_call(rvalue_or_call)
            },
            Statement::Drop(Drop { place, .. }, _) => self.check_place(place),
            Statement::Control(_control, _) => todo!(),
            Statement::Loop(Loop { label: _, block: _, .. }) => todo!(),
            Statement::SwitchInt(_switch_int) => todo!(),
        }
    }

    fn check_rvalue_or_call(&self, rvalue_or_call: &RvalueOrCall) -> syn::Result<()> {
        match rvalue_or_call {
            RvalueOrCall::Rvalue(rvalue) => self.check_rvalue(rvalue),
            RvalueOrCall::Call(call) => self.check_call(call),
            RvalueOrCall::Any(_) => Ok(()),
        }
    }

    fn check_rvalue(&self, rvalue: &Rvalue) -> syn::Result<()> {
        match rvalue {
            Rvalue::Use(RvalueUse { operand, .. })
            | Rvalue::UnaryOp(RvalueUnOp { operand, .. })
            | Rvalue::Repeat(RvalueRepeat { operand, .. }) => self.check_operand(operand),
            Rvalue::Ref(RvalueRef { place, .. })
            | Rvalue::AddressOf(RvalueAddrOf { place, .. })
            | Rvalue::Len(RvalueLen { place, .. })
            | Rvalue::Discriminant(RvalueDiscriminant { place, .. }) => self.check_place(place),
            Rvalue::Cast(RvalueCast { operand, ty, .. }) => {
                self.check_operand(operand)?;
                self.check_type(ty)?;
                Ok(())
            },
            Rvalue::BinaryOp(RvalueBinOp { lhs, rhs, .. }) => {
                self.check_operand(lhs)?;
                self.check_operand(rhs)?;
                Ok(())
            },
            Rvalue::NullaryOp(RvalueNullOp { ty, .. }) => self.check_type(ty),
            Rvalue::Aggregate(agg) => self.check_aggregate(agg),
        }
    }

    fn check_call(&self, call: &Call) -> syn::Result<()> {
        self.check_operand(&call.func)?;
        let operands = match &call.operands {
            CallOperands::Ordered(ParenthesizedOperands { operands, .. })
            | CallOperands::Unordered(BracedOperands { operands, .. }) => operands,
        };
        for operand in operands.iter() {
            self.check_operand(operand)?;
        }
        Ok(())
    }

    fn check_operand(&self, operand: &Operand) -> syn::Result<()> {
        match operand {
            Operand::Copy(OperandCopy { place, .. }) | Operand::Move(OperandMove { place, .. }) => {
                self.check_place(place)
            },
            Operand::Constant(konst) => self.check_const(konst),
        }
    }

    fn check_const(&self, konst: &Const) -> syn::Result<()> {
        match konst {
            Const::Lit(_) => Ok(()),
            Const::Path(type_path) => {
                if let Some(qself) = &type_path.qself {
                    self.check_type(&qself.ty)?;
                }
                self.check_path(&type_path.path)?;
                Ok(())
            },
        }
    }

    fn check_place(&self, place: &Place) -> syn::Result<()> {
        match place {
            Place::Local(PlaceLocal::Local(local)) => self.check_local(local),
            Place::Local(PlaceLocal::Underscore(_) | PlaceLocal::SelfValue(_)) => Ok(()),

            Place::Paren(PlaceParen { place, .. })
            | Place::Deref(PlaceDeref { place, .. })
            | Place::Field(PlaceField { place, .. })
            | Place::Subslice(PlaceSubslice { place, .. })
            | Place::DownCast(PlaceDowncast { place, .. }) => self.check_place(place),
            Place::Index(PlaceIndex { place, index, .. }) => {
                self.check_place(place)?;
                self.check_local(index)?;
                Ok(())
            },
            Place::ConstIndex(PlaceConstIndex {
                place,
                index,
                min_length,
                ..
            }) => {
                self.check_place(place)?;
                if index.index >= min_length.index {
                    return Err(syn::Error::new_spanned(
                        index,
                        CheckError::ConstantIndexOutOfBound(index.index, min_length.index),
                    ));
                }
                Ok(())
            },
        }
    }

    fn check_local(&self, place: &Ident) -> syn::Result<()> {
        self.symbols.get_local(place)?;
        Ok(())
    }

    fn check_aggregate(&self, agg: &RvalueAggregate) -> syn::Result<()> {
        match agg {
            RvalueAggregate::Array(AggregateArray { operands, .. }) => {
                // self.check_type(ty)?;
                for operand in operands.operands.iter() {
                    self.check_operand(operand)?;
                }
                Ok(())
            },
            RvalueAggregate::Tuple(AggregateTuple { operands }) => {
                for operand in operands.operands.iter() {
                    self.check_operand(operand)?;
                }
                Ok(())
            },
            RvalueAggregate::Adt(AggregateAdt { adt, fields }) => {
                self.check_path(adt)?;
                for field in fields.fields.iter() {
                    self.check_operand(&field.operand)?;
                }
                Ok(())
            },
            RvalueAggregate::RawPtr(AggregateRawPtr { ty, ptr, metadata, .. }) => {
                self.check_type(&ty.ty)?;
                self.check_operand(ptr)?;
                self.check_operand(metadata)?;
                Ok(())
            },
        }
    }

    fn check_type(&self, ty: &Type) -> syn::Result<()> {
        match ty {
            Type::Never(_) => Ok(()),
            Type::Reference(TypeReference { region, ty, .. }) => {
                self.check_region(*region)?;
                self.check_type(ty)?;
                Ok(())
            },
            Type::Array(TypeArray { ty, .. })
            | Type::Group(TypeGroup { ty, .. })
            | Type::Paren(TypeParen { ty, .. })
            | Type::Slice(TypeSlice { ty, .. })
            | Type::Ptr(TypePtr { ty, .. }) => self.check_type(ty),
            Type::Path(TypePath { qself, path }) => {
                if let Some(qself) = qself {
                    self.check_type(&qself.ty)?;
                }
                self.check_path(path)?;
                Ok(())
            },
            Type::Tuple(TypeTuple { tys, .. }) => tys.iter().map(|ty| self.check_type(ty)).collect(),
            Type::TyVar(TypeVar { ident, .. }) => {
                _ = self.symbols.get_ty_var(ident)?;
                Ok(())
            },
        }
    }

    fn check_path(&self, path: &Path) -> syn::Result<()> {
        if let Some(ident) = path.as_ident() {
            if !crate::is_primitive(ident) {
                _ = self.symbols.get_type(ident)?;
            }
        }
        Ok(())
    }

    fn check_region(&self, _region: Option<Region>) -> syn::Result<()> {
        Ok(())
    }
}
