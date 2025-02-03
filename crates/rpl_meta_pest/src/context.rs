use std::path::PathBuf;

use crate::idx::RPLIdx;
use rustc_data_structures::fx::FxHashMap;

#[derive(Default)]
pub struct RPLMetaContext<'mctx> {
    pub path2id: FxHashMap<PathBuf, RPLIdx>,
    pub contents: FxHashMap<RPLIdx, String>,
    _marker: std::marker::PhantomData<&'mctx ()>,
}

impl<'mctx> RPLMetaContext<'mctx> {
    /// Request a tree id for the given path.
    /// If the path already has an id, return it.
    /// Otherwise, create a new id, insert it into the path2id map, and return it.
    pub fn request_rpl_idx(&mut self, path: PathBuf) -> RPLIdx {
        if let Some(&id) = self.path2id.get(&path) {
            id
        } else {
            let id: RPLIdx = self.path2id.len().into();
            self.path2id.insert(path.clone(), id);
            id
        }
    }
}
