use crate::idx::RPLIdx;
use crate::meta::RPLMeta;
use rustc_data_structures::fx::FxHashMap;
use std::cell::Cell;
use std::path::Path;

/// Provides a context for the meta data of the RPL multi-files/modularity.
#[derive(Default)]
pub struct RPLMetaContext<'mctx> {
    pub path2id: FxHashMap<&'mctx Path, RPLIdx>,
    pub contents: FxHashMap<RPLIdx, &'mctx str>,
    metas: FxHashMap<RPLIdx, RPLMeta<'mctx>>,
    active_path: Cell<Option<&'mctx Path>>,
}

impl<'mctx> RPLMetaContext<'mctx> {
    /// Request a tree id for the given path.
    /// If the path already has an id, return it.
    /// Otherwise, create a new id, insert it into the path2id map, and return it.
    pub fn request_rpl_idx(&mut self, path: &'mctx Path) -> RPLIdx {
        if let Some(&id) = self.path2id.get(&path) {
            id
        } else {
            let id: RPLIdx = self.path2id.len().into();
            self.path2id.insert(path, id);
            id
        }
    }

    /// Set the active path.
    pub fn set_active_path(&self, path: &'mctx Path) {
        self.active_path.set(Some(path));
    }

    /// Get the active path.
    pub fn get_active_path(&self) -> &'mctx Path {
        self.active_path
            .get()
            .unwrap_or_else(|| panic!("Active path is not set."))
    }

    /// Clear the active path.
    pub fn clear_active_path(&self) {
        self.active_path.set(None);
    }

    pub fn add_meta(&mut self, idx: RPLIdx, meta: RPLMeta<'mctx>) {
        self.metas.insert(idx, meta);
    }
}
