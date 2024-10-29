use std::fmt;

use super::*;

impl fmt::Debug for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}[{:?}]", self.block, self.statement_index)
    }
}

impl fmt::Debug for Place<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.projection {
            [] => self.local.fmt(f),
            [projection @ .., last] => fmt_projection(
                f,
                Place {
                    local: self.local,
                    projection,
                },
                last,
            ),
        }
    }
}

fn fmt_projection<'tcx>(f: &mut fmt::Formatter<'_>, place: Place<'tcx>, proj: &PlaceElem<'tcx>) -> fmt::Result {
    struct FromEnd(bool);
    impl fmt::Display for FromEnd {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if self.0 {
                f.write_str("-")
            } else {
                Ok(())
            }
        }
    }
    match proj {
        PlaceElem::Deref => write!(f, "(*{place:?})"),
        PlaceElem::Field(field) => write!(f, "({place:?}.{field:?})"),
        PlaceElem::Index(local) => write!(f, "({place:?}[{local:?}])"),
        &PlaceElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => {
            let from_end = FromEnd(from_end);
            write!(f, "({place:?}[{from_end}{offset} of {min_length}])")
        },
        &PlaceElem::Subslice { from, to, from_end } => {
            let from_end = FromEnd(from_end);
            write!(f, "({place:?}[{from}:{from_end}{to}])")
        },
        PlaceElem::Downcast(variant) => write!(f, "({place:?} as {variant})"),
        PlaceElem::OpaqueCast(ty) | PlaceElem::Subtype(ty) => write!(f, "({place:?} as {ty:?})"),
    }
}

impl fmt::Debug for ItemPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            [] => f.write_str(" "),
            [first, rest @ ..] => {
                fmt::Display::fmt(first, f)?;
                for path in rest {
                    write!(f, "::{path}")?;
                }
                Ok(())
            },
        }
    }
}

impl fmt::Display for ItemPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl fmt::Debug for Path<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Item(path) => path.fmt(f),
            Self::TypeRelative(ty, path) => write!(f, "< {ty:?} >::{path}"),
            Self::LangItem(lang_item) => write!(f, "#[lang = \"{}\"]", lang_item.name()),
        }
    }
}

impl fmt::Debug for Ty<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl fmt::Debug for TyKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::TyVar(ty_var) => ty_var.fmt(f),
            Self::Array(ty, len) => write!(f, "[{ty:?}; {len:?}]"),
            Self::Slice(ty) => write!(f, "[{ty:?}]"),
            Self::Tuple(tys) => {
                f.write_str("(")?;
                for ty in tys {
                    write!(f, "{ty:?}, ")?;
                }
                f.write_str(")")
            },
            Self::Ref(region, ty, mir::Mutability::Not) => write!(f, "&{region} {ty:?}"),
            Self::Ref(region, ty, mir::Mutability::Mut) => write!(f, "&{region}mut {ty:?}"),
            Self::RawPtr(ty, mutability) => write!(f, "*{} {ty:?}", mutability.ptr_str()),
            Self::Adt(path, args) => {
                write!(f, "{path:?}{args:?}")
            },
            Self::Uint(uint) => uint.fmt(f),
            Self::Int(int) => int.fmt(f),
            Self::Float(float) => float.fmt(f),
            Self::Bool => f.write_str("bool"),
            Self::Str => f.write_str("str"),
            Self::FnDef(path, args) => write!(f, "{path:?}{args:?}"),
            Self::Alias(_alias_kind, path, args) => write!(f, "{path:?}{args:?}"),
        }
    }
}

impl fmt::Debug for GenericArgsRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            [] => Ok(()),
            [arg] => write!(f, "< {arg:?} >"),
            [first, args @ ..] => {
                if self.0.is_empty() {
                    return Ok(());
                }
                write!(f, "< {first:?}")?;
                for arg in args {
                    f.write_str(",")?;
                    arg.fmt(f)?;
                }
                f.write_str(" >")
            },
        }
    }
}

impl fmt::Debug for GenericArgKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifetime(region) => region.fmt(f),
            Self::Type(ty) => ty.fmt(f),
            Self::Const(konst) => konst.fmt(f),
        }
    }
}

impl fmt::Debug for RegionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReStatic => f.write_str("'static"),
            Self::ReAny => f.write_str("'_"),
        }
    }
}

impl fmt::Display for RegionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReStatic => f.write_str("'static"),
            Self::ReAny => Ok(()),
        }
    }
}

impl fmt::Debug for StatementKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assign(place, rvalue) => write!(f, "{place:?} = {rvalue:?}"),
            Self::Init(place) => write!(f, "{place:?} = _"),
        }
    }
}

impl fmt::Debug for TerminatorKind<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
            } => {
                if let Some(destination) = destination {
                    write!(f, "{destination:?} = ")?
                }
                write!(f, "{func:?}{args:?} -> {target:?}")
            },
            TerminatorKind::Drop { place, target } => {
                write!(f, "drop({place:?}) -> {target:?}")
            },
            TerminatorKind::SwitchInt { operand, targets } => write!(f, "switchInt({operand:?}) -> {targets:?}"),
            TerminatorKind::Goto(basic_block) => write!(f, "goto {basic_block:?}"),
            TerminatorKind::Return => f.write_str("return"),
            TerminatorKind::PatEnd => f.write_str("end"),
        }
    }
}

impl fmt::Debug for SwitchTargets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut targets = f.debug_list();
        for (val, bb) in &self.targets {
            targets.entry_with(|f| write!(f, "{val:?} -> {bb:?}"));
        }
        if let Some(bb) = self.otherwise {
            targets.entry_with(|f| write!(f, "otherwise -> {bb:?}"));
        }
        targets.finish()
    }
}

impl fmt::Debug for IntValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.ty {
            IntTy::Bool => write!(f, "{}", self.value != 0),
            IntTy::Int(ty) => write!(f, "{}_{}", self.value, ty.name_str()),
            IntTy::NegInt(ty) => write!(f, "-{}_{}", self.value, ty.name_str()),
            IntTy::Uint(ty) => write!(f, "{}_{}", self.value, ty.name_str()),
        }
    }
}

impl fmt::Debug for Rvalue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Use(operand) => operand.fmt(f),
            Self::Repeat(elem, len) => write!(f, "[{elem:?}; {len:?}]"),
            Self::Ref(region, bor, place) => write!(f, "&{region}{}{place:?}", bor.mutability().prefix_str()),
            Self::RawPtr(mutability, place) => write!(f, "&raw {} {place:?}", mutability.ptr_str()),
            Self::Len(place) => f.debug_tuple("Len").field(place).finish(),
            Self::Cast(cast_kind, operand, ty) => write!(f, "{operand:?} as {ty:?} ({cast_kind:?})"),
            Self::BinaryOp(op, box [lhs, rhs]) => write!(f, "{op:?}({lhs:?}, {rhs:?})"),
            Self::NullaryOp(op, ty) => write!(f, "{op:?}({ty:?})"),
            Self::UnaryOp(op, operand) => write!(f, "{op:?}({operand:?}"),
            Self::Discriminant(place) => f.debug_tuple("discriminant").field(place).finish(),
            Self::Aggregate(agg_kind, args) => format_aggregate(agg_kind, args, f),
            Self::ShallowInitBox(operand, ty) => write!(f, "Box< {ty:?} >({operand:?})"),
            Self::CopyForDeref(place) => write!(f, "&(*{place:?})"),
        }
    }
}

fn format_aggregate<'tcx>(
    agg_kind: &AggKind<'tcx>,
    operands: &[Operand<'tcx>],
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    fn fmt_list<T>(
        f: &mut fmt::Formatter<'_>,
        list: impl IntoIterator<Item = T>,
        end: &str,
        mut fmt: impl FnMut(T, &mut fmt::Formatter<'_>) -> fmt::Result,
    ) -> fmt::Result {
        let mut iter = list.into_iter();
        if let Some(first) = iter.next() {
            fmt(first, f)?;
        }
        for elem in iter {
            write!(f, ", ")?;
            fmt(elem, f)?;
        }
        f.write_str(end)
    }
    match agg_kind {
        AggKind::Array => {
            f.write_str("[")?;
            fmt_list(f, operands, "]", fmt::Debug::fmt)
        },
        AggKind::Tuple => {
            f.write_str("(")?;
            fmt_list(f, operands, ")", fmt::Debug::fmt)
        },
        AggKind::Adt(path, None) => {
            write!(f, "{path:?}(")?;
            fmt_list(f, operands, ")", fmt::Debug::fmt)
        },
        AggKind::Adt(path, Some(fields)) => {
            write!(f, "{path:?}{{")?;
            let mut fields = fields.iter();
            fmt_list(f, operands, "}", |operand, f| {
                let field = fields.next().ok_or(std::fmt::Error)?;
                write!(f, "{field}: {operand:?}")
            })
        },
        AggKind::RawPtr(ty, mutability) => {
            write!(f, "*{} {ty:?} from (", mutability.ptr_str())?;
            fmt_list(f, operands, ")", fmt::Debug::fmt)
        },
    }
}

impl fmt::Debug for Operand<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Copy(place) => write!(f, "copy {place:?}"),
            Self::Move(place) => write!(f, "move {place:?}"),
            Self::Constant(konst) => konst.fmt(f),
        }
    }
}

impl fmt::Debug for ConstOperand<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConstVar(const_var) => const_var.fmt(f),
            Self::ScalarInt(scalar) => write!(f, "const {scalar:?}"),
            Self::ZeroSized(ty) => ty.fmt(f),
        }
    }
}

impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Named(sym) => write!(f, "{sym}"),
            Self::Unnamed(field) => field.fmt(f),
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for List<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (begin, non_exhaustive, end) = match self.mode {
            ListMatchMode::Ordered => ("(", "", ")"),
            ListMatchMode::Unordered => ("{", ".., ", "}"),
        };
        match &self.data {
            box [] => write!(f, "{begin}{end}"),
            box [first, rest @ ..] => {
                write!(f, "{begin}{first:?}")?;
                for v in rest {
                    write!(f, ", {v:?}")?;
                }
                write!(f, "{non_exhaustive}{end}")
            },
        }
    }
}

impl fmt::Debug for Patterns<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let new_line = if f.alternate() { "\n" } else { " " };
        let indent = if f.alternate() { "    " } else { "" };
        let mut meta = f.debug_tuple("meta!");
        for (ty_var, _ty_pred) in self.ty_vars.iter_enumerated() {
            meta.field_with(|f| write!(f, "{ty_var:?}:ty"));
        }
        for (const_var, ty) in self.const_vars.iter_enumerated() {
            meta.field_with(|f| write!(f, "const {const_var:?}: {ty:?}"));
        }
        meta.finish()?;
        write!(f, ";{new_line}")?;
        for (local, ty) in self.locals.iter_enumerated() {
            write!(f, "let {local:?}: {ty:?} ;{new_line}")?;
        }
        for (bb, block) in self.basic_blocks.iter_enumerated() {
            write!(f, "{bb:?}: {{{new_line}")?;
            for statement in &block.statements {
                write!(f, "{indent}{statement:?};{new_line}")?;
            }
            if let Some(terminator) = &block.terminator {
                write!(f, "{indent}{terminator:?};{new_line}")?;
            }
            write!(f, "}}{new_line}")?;
        }
        Ok(())
    }
}
