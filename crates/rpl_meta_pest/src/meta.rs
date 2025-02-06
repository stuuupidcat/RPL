use crate::collect_elems_separated_by_comma;
use crate::context::RPLMetaContext;
use crate::error::{RPLMetaError, RPLMetaResult};
use crate::idx::RPLIdx;
use crate::symbol_table::{DiagSymbolTable, SymbolTable};
use crate::utils::Ident;
use colored::Colorize;
use parser::{pairs, parse_main};
use rustc_data_structures::fx::FxHashMap;
use std::path::Path;

/// Meta data of a single rpl file.
pub struct RPLMeta<'i> {
    /// Absolute path to the rpl file
    pub path: &'i Path,
    /// RPL Idx
    pub idx: RPLIdx,
    /// The name of the rpl file
    pub name: &'i str,
    /// The symbol table of the util block
    util_symbol_tables: FxHashMap<&'i str, SymbolTable<'i>>,
    /// The symbol table of the patt block
    patt_symbol_tables: FxHashMap<&'i str, SymbolTable<'i>>,
    /// The symbol table of the diag block
    diag_symbol_tables: FxHashMap<&'i str, DiagSymbolTable<'i>>,
    /// errors
    pub errors: Vec<RPLMetaError<'i>>,
}

impl<'i> RPLMeta<'i> {
    // the input in the arg has already been leaked by Box::leak
    pub fn parse_and_collect<'r>(
        path: &'i Path,
        input: &'i str,
        mctx: &'r mut RPLMetaContext<'i>,
    ) -> RPLMetaResult<'i, Self> {
        mctx.set_active_path(&path);

        let rpl_idx = mctx.request_rpl_idx(path);
        mctx.contents.insert(rpl_idx, input);

        let main = parse_main(input, &path).map_err(|error| RPLMetaError::ParseError { error })?;
        let main: &mut pairs::main<'i> = Box::leak(Box::new(main));
        let res = Self::collect(path, main, rpl_idx, mctx);

        mctx.clear_active_path();
        Ok(res)
    }

    /// Collect the meta data of a parsed rpl file
    pub fn collect(path: &'i Path, main: &'i pairs::main<'i>, idx: RPLIdx, mctx: &RPLMetaContext<'i>) -> Self {
        let mut errors = Vec::new();
        // Collect the pattern name of the rpl file.
        let name = Self::collect_rpl_pattern_name(&main);
        // Collect the blocks.
        let (utils, patts, diags) = collect_blocks(&main);
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

    fn collect_rpl_pattern_name(main: &pairs::main<'i>) -> &'i str {
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
