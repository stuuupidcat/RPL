use super::*;

pub trait PatternVisitor<'tcx>: Sized {
    fn visit_local(&mut self, _local: LocalIdx) {}
    fn visit_const_value(&mut self, _const_var: ConstValue) {}
    fn visit_ty_var(&mut self, _ty_var: TyVarIdx) {}
    fn visit_const_var(&mut self, _const_var: ConstVarIdx) {}

    fn visit_pattern(&mut self, pattern: &Pattern<'tcx>) {
        pattern.visit_with(self);
    }
    fn visit_place(&mut self, place: Place<'tcx>) {
        place.visit_with(self);
    }
    fn visit_rvalue(&mut self, rvalue: &Rvalue<'tcx>) {
        rvalue.visit_with(self);
    }
    fn visit_operand(&mut self, operand: &Operand<'tcx>) {
        operand.visit_with(self);
    }
    fn visit_const_operand(&mut self, const_operand: &ConstOperand<'tcx>) {
        const_operand.visit_with(self);
    }
    fn visit_statement(&mut self, statement: &StatementKind<'tcx>) {
        statement.visit_with(self);
    }
    fn visit_terminator(&mut self, terminator: &TerminatorKind<'tcx>) {
        terminator.visit_with(self);
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>) {
        ty.visit_with(self);
    }
    fn visit_const(&mut self, konst: Const<'tcx>) {
        konst.visit_with(self);
    }
    fn visit_generic_args(&mut self, args: GenericArgsRef<'tcx>) {
        args.visit_with(self);
    }
    fn visit_generic_arg(&mut self, arg: GenericArgKind<'tcx>) {
        arg.visit_with(self);
    }
    fn visit_path(&mut self, path: &Path<'tcx>) {
        path.visit_with(self);
    }
}

pub trait PatternVisitable<'tcx>: PatternSuperVisitable<'tcx> {
    fn visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        self.super_visit_with(vis);
    }
}

pub trait PatternSuperVisitable<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V);
}

impl<'tcx, P: PatternSuperVisitable<'tcx>> PatternVisitable<'tcx> for P {}

impl<'tcx> PatternSuperVisitable<'tcx> for Pattern<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match self.kind {
            PatternKind::TyVar => {},
            PatternKind::ConstVar => {},
            PatternKind::Init(local) => vis.visit_local(local),
            PatternKind::Statement(ref statement) => vis.visit_statement(statement),
            PatternKind::Terminator(ref terminator) => vis.visit_terminator(terminator),
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for Place<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        vis.visit_local(self.local);
        // TODO: visit place projections
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for Ty<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match self.kind() {
            &TyKind::TyVar(ty_var) => vis.visit_ty_var(ty_var),
            &TyKind::Array(ty, konst) => {
                vis.visit_ty(ty);
                vis.visit_const(konst);
            },
            &TyKind::Slice(ty) => vis.visit_ty(ty),
            TyKind::Tuple(tys) => tys.iter().for_each(|&ty| vis.visit_ty(ty)),
            &TyKind::Ref(_region, ty, _) => {
                vis.visit_ty(ty);
            },
            &TyKind::RawPtr(ty, _) => vis.visit_ty(ty),
            &TyKind::Adt(_, args) => vis.visit_generic_args(args),
            TyKind::Uint(_) | TyKind::Int(_) | TyKind::Float(_) => {},
            &TyKind::FnDef(ref path, args) => {
                vis.visit_path(path);
                vis.visit_generic_args(args);
            },
            &TyKind::Alias(_, ref path, args) => {
                vis.visit_path(path);
                vis.visit_generic_args(args);
            },
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for GenericArgsRef<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        self.iter().for_each(|&arg| vis.visit_generic_arg(arg));
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for Const<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match *self.kind() {
            ConstKind::ConstVar(const_var) => vis.visit_const_var(const_var),
            ConstKind::Value(ty, scalar) => {
                vis.visit_ty(ty);
                vis.visit_const_value(ConstValue::Scalar(scalar.into()));
            },
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for GenericArgKind<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match *self {
            GenericArgKind::Lifetime(_region) => {},
            GenericArgKind::Type(ty) => vis.visit_ty(ty),
            GenericArgKind::Const(konst) => vis.visit_const(konst),
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for Path<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match *self {
            Path::Item(_) | Path::LangItem(_) => {},
            Path::TypeRelative(ty, _) => vis.visit_ty(ty),
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for ConstOperand<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match *self {
            ConstOperand::Ty(ty, konst) => {
                vis.visit_ty(ty);
                vis.visit_const(konst);
            },
            ConstOperand::Val(value, ty) => {
                vis.visit_const_value(value);
                vis.visit_ty(ty);
            },
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for Rvalue<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match self {
            Rvalue::Use(operand) | Rvalue::UnaryOp(_, operand) => vis.visit_operand(operand),
            &Rvalue::Repeat(ref operand, konst) => {
                vis.visit_operand(operand);
                vis.visit_const(konst);
            },
            &Rvalue::Ref(_region, _borrow_kind, place) => vis.visit_place(place),
            &Rvalue::AddressOf(_mutability, place) => vis.visit_place(place),
            &Rvalue::Len(place) | &Rvalue::Discriminant(place) | &Rvalue::CopyForDeref(place) => {
                vis.visit_place(place);
            },
            &Rvalue::Cast(_, ref operand, ty) | &Rvalue::ShallowInitBox(ref operand, ty) => {
                vis.visit_operand(operand);
                vis.visit_ty(ty);
            },
            Rvalue::BinaryOp(_op, box [lhs, rhs]) => {
                vis.visit_operand(lhs);
                vis.visit_operand(rhs);
            },
            &Rvalue::NullaryOp(_op, ty) => vis.visit_ty(ty),
            Rvalue::Aggregate(_agg_kind, operands) => operands.iter().for_each(|operand| vis.visit_operand(operand)),
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for Operand<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match self {
            &Operand::Copy(place) | &Operand::Move(place) => vis.visit_place(place),
            Operand::Constant(const_operand) => vis.visit_const_operand(const_operand),
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for StatementKind<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match self {
            &StatementKind::Assign(place, ref rvalue) => {
                vis.visit_place(place);
                vis.visit_rvalue(rvalue);
            },
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for TerminatorKind<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match *self {
            TerminatorKind::Call {
                ref func,
                ref args,
                destination,
            } => {
                vis.visit_operand(func);
                for arg in &args.data {
                    vis.visit_operand(arg);
                }
                vis.visit_place(destination);
            },
            TerminatorKind::Drop { place } => vis.visit_place(place),
        }
    }
}
