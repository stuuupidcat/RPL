use rustc_middle::mir::visit::{PlaceContext, Visitor};
use rustc_middle::mir::{self};

use crate::BlockDataDepGraph;

impl<'tcx> Visitor<'tcx> for BlockDataDepGraph<mir::Local> {
    fn visit_local(&mut self, local: mir::Local, pcx: PlaceContext, location: mir::Location) {
        self.access_local(local, pcx, location.statement_index);
    }
    fn visit_statement(&mut self, statement: &mir::Statement<'tcx>, location: mir::Location) {
        self.super_statement(statement, location);
        self.update_deps(location.statement_index);
    }
    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>, location: mir::Location) {
        self.super_terminator(terminator, location);
        self.update_deps(location.statement_index);
        self.update_dep_end();
    }
}
