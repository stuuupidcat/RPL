use crate::context::RPLMetaContext;
use crate::error::RPLMetaError;
use crate::idx::RPLIdx;
use crate::symbol_table::{DiagSymbolTable, SymbolTable};
use colored::Colorize;
use parser::pairs;
use rustc_data_structures::fx::FxHashMap;
use std::path::Path;

/// Meta data of a single rpl file.
pub struct RPLMeta<'mctx> {
    /// Absolute path to the rpl file
    pub path: &'mctx Path,
    /// RPL Idx
    pub idx: RPLIdx,
    /// The name of the rpl file
    pub name: &'mctx str,
    /// The symbol table of the util block
    util_symbol_tables: FxHashMap<&'mctx str, SymbolTable<'mctx>>,
    /// The symbol table of the patt block
    patt_symbol_tables: FxHashMap<&'mctx str, SymbolTable<'mctx>>,
    /// The symbol table of the diag block
    diag_symbol_tables: FxHashMap<&'mctx str, DiagSymbolTable<'mctx>>,
    /// errors
    pub errors: Vec<RPLMetaError<'mctx>>,
}

impl<'mctx> RPLMeta<'mctx> {
    /// Collect the meta data of a parsed rpl file
    pub fn collect(
        path: &'mctx Path,
        main: &'mctx pairs::main<'mctx>,
        idx: RPLIdx,
        mctx: &RPLMetaContext<'mctx>,
    ) -> Self {
        let mut errors = Vec::new();
        // Collect the pattern name of the rpl file.
        let name = Self::collect_rpl_pattern_name(main);
        // Collect the blocks.
        let (utils, patts, diags) = collect_blocks(main);
        // Collect the symbol table of the util blocks.
        let util_items = utils.iter().flat_map(|util| util.get_matched().3.iter_matched());
        let util_symbol_tables = SymbolTable::collect_symbol_tables(mctx, util_items, &mut errors);
        // Collect the symbol table of the patt blocks.
        let patt_items = patts.iter().flat_map(|patt| patt.get_matched().3.iter_matched());
        let patt_symbol_tables = SymbolTable::collect_symbol_tables(mctx, patt_items, &mut errors);
        let diag_items = diags.iter().flat_map(|diag| diag.get_matched().2.iter_matched());
        let diag_symbol_tables = DiagSymbolTable::collect_symbol_tables(mctx, diag_items, &mut errors);
        RPLMeta {
            path,
            name,
            idx,
            util_symbol_tables,
            patt_symbol_tables,
            diag_symbol_tables,
            errors,
        }
    }

    fn collect_rpl_pattern_name(main: &pairs::main<'mctx>) -> &'mctx str {
        let rpl_pattern = main.get_matched().1;
        let rpl_header = rpl_pattern.get_matched().0;
        let name = rpl_header.get_matched().1.span.as_str();
        name
    }
}

impl RPLMeta<'_> {
    pub fn show_error(&self, f: &mut impl std::io::Write) {
        if !self.errors.is_empty() {
            writeln!(
                f,
                "{}",
                format!(
                    "{:?} generated {} error{}.",
                    self.path,
                    self.errors.len(),
                    if self.errors.len() > 1 { "s" } else { "" }
                )
                .red()
                .bold(),
            )
            .unwrap();

            let mut cnt = 1usize;
            for error in &self.errors {
                writeln!(f, "{}. {}", cnt, error).unwrap();
                cnt += 1;
            }
        } else {
            writeln!(f, "{}", format!("No error found in {:?}", self.path).green().bold(),).unwrap();
        }
    }
}

pub fn collect_blocks<'i>(
    main: &'i pairs::main<'i>,
) -> (
    Vec<&'i pairs::utilBlock<'i>>,
    Vec<&'i pairs::pattBlock<'i>>,
    Vec<&'i pairs::diagBlock<'i>>,
) {
    let mut utils = Vec::new();
    let mut patts = Vec::new();
    let mut diags = Vec::new();

    let blocks = main.get_matched().1.get_matched().1;
    let blocks = blocks.iter_matched();

    for block in blocks {
        if let Some(util) = block.utilBlock() {
            utils.push(util);
        } else if let Some(patt) = block.pattBlock() {
            patts.push(patt);
        } else if let Some(diag) = block.diagBlock() {
            diags.push(diag);
        }
    }

    (utils, patts, diags)
}
