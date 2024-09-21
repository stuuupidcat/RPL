use std::fmt;

use super::*;

impl<'tcx> fmt::Debug for Place<'tcx> {
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
            if self.0 { f.write_str("-") } else { Ok(()) }
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

impl<'tcx> fmt::Debug for Path<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Item(path) => path.fmt(f),
            Self::TypeRelative(ty, path) => write!(f, "<{ty:?}>::{path}"),
            Self::LangItem(lang_item) => lang_item.fmt(f),
        }
    }
}

impl<'tcx> fmt::Debug for Ty<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.kind().fmt(f)
    }
}

impl<'tcx> fmt::Debug for TyKind<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::TyVar(ty_var) => ty_var.fmt(f),
            Self::Array(ty, len) => write!(f, "[{ty:?}; {len:?}]"),
            Self::Slice(ty) => write!(f, "[{ty:?}]"),
            Self::Tuple(tys) => {
                let mut dbg = f.debug_tuple("");
                for ty in tys {
                    dbg.field(ty);
                }
                dbg.finish()
            },
            Self::Ref(region, ty, mutability) => write!(f, "&{region}{}{ty:?}", mutability.prefix_str()),
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

impl<'tcx> fmt::Debug for GenericArgsRef<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        f.write_str("<")?;
        for arg in self.0 {
            arg.fmt(f)?;
        }
        f.write_str(">")
    }
}

impl<'tcx> fmt::Debug for GenericArgKind<'tcx> {
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

impl<'tcx> fmt::Debug for StatementKind<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Assign(place, rvalue) => write!(f, "{place:?} = {rvalue:?}"),
        }
    }
}

impl<'tcx> fmt::Debug for TerminatorKind<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call {
                func,
                args,
                destination,
            } => write!(f, "{destination:?} = {func:?}({args:?})"),
            Self::Drop { place } => f.debug_tuple("Drop").field(place).finish(),
        }
    }
}

impl<'tcx> fmt::Debug for Rvalue<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Use(operand) => operand.fmt(f),
            Self::Repeat(elem, len) => write!(f, "[{elem:?}; {len:?}]"),
            Self::Ref(region, bor, place) => write!(f, "&{region}{}{place:?}", bor.mutability().prefix_str()),
            Self::AddressOf(mutability, place) => write!(f, "&raw {} {place:?}", mutability.ptr_str()),
            Self::Len(place) => f.debug_tuple("Len").field(place).finish(),
            Self::Cast(cast_kind, operand, ty) => write!(f, "{operand:?} as {ty:?} ({cast_kind:?})"),
            Self::BinaryOp(op, box [lhs, rhs]) => write!(f, "{op:?}({lhs:?}, {rhs:?}"),
            Self::NullaryOp(op, ty) => write!(f, "{op:?}({ty:?})"),
            Self::UnaryOp(op, operand) => write!(f, "{op:?}({operand:?}"),
            Self::Discriminant(place) => f.debug_tuple("Discriminant").field(place).finish(),
            Self::Aggregate(agg_kind, args) => write!(f, "{agg_kind:?} from {args:?}"),
            Self::ShallowInitBox(operand, ty) => write!(f, "Box<{ty:?}>({operand:?})"),
            Self::CopyForDeref(place) => write!(f, "&(*{place:?})"),
        }
    }
}

impl<'tcx> fmt::Debug for Operand<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Copy(place) => place.fmt(f),
            Self::Move(place) => write!(f, "move {place:?}"),
            Self::Constant(konst) => konst.fmt(f),
        }
    }
}

impl<'tcx> fmt::Debug for ConstOperand<'tcx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ty(ty, Const(ConstKind::ConstVar(const_var))) => write!(f, "const {const_var:?}: {ty:?}"),
            Self::Ty(ty, Const(ConstKind::Value(_ty, scalar))) => write!(f, "{scalar:?}_{ty:?}"),
            Self::Val(ConstValue::ZeroSized, ty) => ty.fmt(f),
            Self::Val(ConstValue::Scalar(scalar), _) => scalar.fmt(f),
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
