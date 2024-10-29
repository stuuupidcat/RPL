use crate::symbol_table::CheckError;
use crate::SymbolTable;
use proc_macro2::Ident;
use rpl_mir_syntax::*;
use rustc_span::Symbol;
use syn::Token;

pub(crate) fn check_mir(mir: &Mir) -> syn::Result<()> {
    rustc_span::create_session_if_not_set_then(rustc_span::edition::LATEST_STABLE_EDITION, |_| {
        CheckCtxt::new().check_mir(mir)
    })
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
        meta.meta.content.iter().try_for_each(|item| self.check_meta_item(item))
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
            Declaration::SelfDecl(self_value) => self.symbols.add_self_value(self_value.clone()),
        }
    }

    fn check_stmt<End>(&self, stmt: &Statement<End>) -> syn::Result<()> {
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
            Statement::Call(CallIgnoreRet { call, .. }, _) => self.check_call(call),
            Statement::Drop(Drop { place, .. }, _) => self.check_place(place),
            Statement::Control(control, _) => self.check_control(control),
            Statement::Loop(Loop { label, block, .. }) => self.check_loop(label.as_ref(), block),
            Statement::SwitchInt(switch_int) => self.check_switch_int(switch_int),
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
            | Rvalue::RawPtr(RvalueRawPtr { place, .. })
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
            Operand::Constant(konst) => self.check_const_operand(konst),
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

    fn check_const_operand(&self, konst: &ConstOperand) -> syn::Result<()> {
        match konst {
            ConstOperand::Lit(_) => Ok(()),
            ConstOperand::Path(type_path) => {
                if let Some(qself) = &type_path.qself {
                    self.check_type(&qself.ty)?;
                }
                self.check_path(&type_path.path)?;
                Ok(())
            },
            ConstOperand::LangItem(lang_item) => self.check_lang_item(lang_item),
        }
    }

    fn check_lang_item(&self, lang_item: &LangItem) -> syn::Result<()> {
        let value = lang_item.item.value();
        rustc_hir::LangItem::from_name(Symbol::intern(&value))
            .ok_or_else(|| syn::Error::new_spanned(lang_item, CheckError::UnknownLangItem(value)))?;
        if let Some(args) = &lang_item.args {
            args.args.iter().try_for_each(|arg| self.check_generic_arg(arg))?;
        }
        Ok(())
    }

    fn check_place(&self, place: &Place) -> syn::Result<()> {
        match place {
            Place::Local(PlaceLocal::Local(local)) => self.check_local(local),
            &Place::Local(PlaceLocal::SelfValue(self_value)) => self.check_self_value(self_value),

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

    fn check_self_value(&self, self_value: Token![self]) -> syn::Result<()> {
        self.symbols.get_self_value(self_value.span)?;
        Ok(())
    }

    fn check_local(&self, local: &Ident) -> syn::Result<()> {
        self.symbols.get_local(local)?;
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
            RvalueAggregate::AdtTuple(AggregateAdtTuple { adt, fields, .. }) => {
                self.check_path(adt)?;
                for field in fields.operands.iter() {
                    self.check_operand(field)?;
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
            Type::Tuple(TypeTuple { tys, .. }) => tys.iter().try_for_each(|ty| self.check_type(ty)),
            Type::TyVar(TypeVar { ident, .. }) => {
                _ = self.symbols.get_ty_var(ident)?;
                Ok(())
            },
            Type::LangItem(lang_item) => self.check_lang_item(lang_item),
        }
    }

    fn check_path(&self, path: &Path) -> syn::Result<()> {
        if let Some(ident) = path.as_ident() {
            if !crate::is_primitive(ident) {
                _ = self.symbols.get_type(ident)?;
            }
        } else {
            for segment in &path.segments {
                self.check_generic_args(&segment.arguments)?;
            }
        }
        Ok(())
    }

    fn check_generic_args(&self, args: &PathArguments) -> syn::Result<()> {
        match args {
            PathArguments::None => Ok(()),
            PathArguments::AngleBracketed(AngleBracketedGenericArguments { args, .. }) => {
                args.iter().try_for_each(|arg| self.check_generic_arg(arg))
            },
        }
    }

    fn check_generic_arg(&self, arg: &GenericArgument) -> syn::Result<()> {
        match arg {
            &GenericArgument::Region(region) => self.check_region(Some(region)),
            GenericArgument::Type(ty) => self.check_type(ty),
            GenericArgument::Const(GenericConst { konst, .. }) => self.check_const(konst),
        }
    }

    fn check_region(&self, _region: Option<Region>) -> syn::Result<()> {
        Ok(())
    }

    fn check_control(&self, _control: &Control) -> syn::Result<()> {
        Ok(())
    }

    fn check_loop(&self, _label: Option<&syn::Label>, block: &Block) -> syn::Result<()> {
        self.check_block(block)
    }

    fn check_block(&self, block: &Block) -> syn::Result<()> {
        block.statements.iter().try_for_each(|stmt| self.check_stmt(stmt))
    }

    fn check_switch_int(&self, switch_int: &SwitchInt) -> syn::Result<()> {
        self.check_operand(&switch_int.operand)?;
        let mut has_otherwise = false;
        for SwitchTarget { value, body, .. } in &switch_int.targets {
            if let SwitchValue::Underscore(_) = value {
                if has_otherwise {
                    return Err(syn::Error::new_spanned(value, CheckError::MultipleOtherwiseInSwitchInt));
                }
                has_otherwise = true;
            }
            self.check_switch_value(value)?;
            self.check_switch_body(body)?;
        }
        Ok(())
    }

    fn check_switch_body(&self, body: &SwitchBody) -> syn::Result<()> {
        match body {
            SwitchBody::Block(block) => self.check_block(block),
            SwitchBody::Statement(stmt, _) => self.check_stmt(stmt),
        }
    }

    fn check_switch_value(&self, value: &SwitchValue) -> syn::Result<()> {
        match value {
            SwitchValue::Bool(_) | SwitchValue::Underscore(_) => {},
            SwitchValue::Int(int) => {
                if int.suffix().trim_start_matches('_').is_empty() {
                    return Err(syn::Error::new_spanned(int, CheckError::MissingSuffixInSwitchInt));
                }
            },
        }
        Ok(())
    }
}
