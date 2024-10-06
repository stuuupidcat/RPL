use super::*;

pub trait PatternVisitor<'tcx>: Sized {
    fn visit_local(&mut self, _local: LocalIdx) {}
    fn visit_scalar_int(&mut self, _scalar_int: IntValue) {}
    fn visit_ty_var(&mut self, _ty_var: TyVarIdx) {}
    fn visit_const_var(&mut self, _const_var: ConstVarIdx) {}

    fn visit_basic_block(&mut self, block: &BasicBlockData<'tcx>) {
        block.visit_with(self);
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
    fn visit_switch_targets(&mut self, targets: &SwitchTargets) {
        targets.visit_with(self);
    }

    fn visit_ty(&mut self, ty: Ty<'tcx>) {
        ty.visit_with(self);
    }
    fn visit_const(&mut self, konst: Const) {
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
            TyKind::Uint(_) | TyKind::Int(_) | TyKind::Float(_) | TyKind::Str | TyKind::Bool => {},
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

impl<'tcx> PatternSuperVisitable<'tcx> for Const {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        match *self {
            Const::ConstVar(const_var) => vis.visit_const_var(const_var),
            Const::Value(int_value) => vis.visit_scalar_int(int_value),
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
            ConstOperand::ConstVar(const_var) => vis.visit_const_var(const_var),
            ConstOperand::ScalarInt(int_value) => vis.visit_scalar_int(int_value),
            ConstOperand::ZeroSized(ty) => vis.visit_ty(ty),
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for BasicBlockData<'tcx> {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, vis: &mut V) {
        for statement in &self.statements {
            vis.visit_statement(statement);
        }
        if let Some(terminator) = &self.terminator {
            vis.visit_terminator(terminator);
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
        match *self {
            StatementKind::Assign(place, ref rvalue) => {
                vis.visit_place(place);
                vis.visit_rvalue(rvalue);
            },
            StatementKind::Init(place) => vis.visit_place(place),
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
                target: _,
            } => {
                vis.visit_operand(func);
                for arg in &args.data {
                    vis.visit_operand(arg);
                }
                if let Some(destination) = destination {
                    vis.visit_place(destination);
                }
            },
            TerminatorKind::Drop { place, target: _ } => vis.visit_place(place),
            TerminatorKind::SwitchInt {
                ref operand,
                ref targets,
            } => {
                vis.visit_operand(operand);
                vis.visit_switch_targets(targets);
            },
            TerminatorKind::Goto(_) | TerminatorKind::Return => {},
        }
    }
}

impl<'tcx> PatternSuperVisitable<'tcx> for SwitchTargets {
    fn super_visit_with<V: PatternVisitor<'tcx>>(&self, _vis: &mut V) {}
}
