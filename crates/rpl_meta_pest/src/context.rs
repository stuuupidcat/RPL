use crate::arena::Arena;
use crate::idx::RPLIdx;
use crate::meta::RPLMeta;
use parser::pairs;
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::sync::WorkerLocal;
use std::cell::Cell;
use std::path::Path;

/// Provides a context for the meta data of the RPL multi-files/modularity.
pub struct RPLMetaContext<'mctx> {
    pub arena: &'mctx WorkerLocal<Arena<'mctx>>,
    pub path2id: FxHashMap<&'mctx Path, RPLIdx>,
    pub id2path: FxHashMap<RPLIdx, &'mctx Path>,
    pub contents: FxHashMap<RPLIdx, &'mctx str>,
    pub syntax_trees: FxHashMap<RPLIdx, &'mctx pairs::main<'mctx>>,
    pub metas: FxHashMap<RPLIdx, RPLMeta<'mctx>>,
    active_path: Cell<Option<&'mctx Path>>,
}

impl<'mctx> RPLMetaContext<'mctx> {
    pub fn new(arena: &'mctx WorkerLocal<Arena<'mctx>>) -> Self {
        Self {
            arena,
            path2id: FxHashMap::default(),
            id2path: FxHashMap::default(),
            contents: FxHashMap::default(),
            syntax_trees: FxHashMap::default(),
            metas: FxHashMap::default(),
            active_path: Cell::new(None),
        }
    }

    /// Request a tree id for the given path.
    /// If the path already has an id, return it.
    /// Otherwise, create a new id, insert it into the path2id map, and return it.
    pub fn request_rpl_idx(&mut self, path: &'mctx Path) -> RPLIdx {
        if let Some(&id) = self.path2id.get(path) {
            id
        } else {
            // FIXME: Is this allocation necessary?
            let path = self.arena.alloc(path);
            let id: RPLIdx = self.path2id.len().into();
            self.path2id.insert(path, id);
            self.id2path.insert(id, path);
            id
        }
    }

    /// Set the active path.
    pub fn set_active_path(&self, path: Option<&'mctx Path>) {
        self.active_path.set(path);
    }

    /// Get the active path.
    pub fn get_active_path(&self) -> &'mctx Path {
        self.active_path
            .get()
            .unwrap_or_else(|| panic!("Active path is not set."))
    }
}
