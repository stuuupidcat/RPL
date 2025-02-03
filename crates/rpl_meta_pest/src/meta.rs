use parser::{parse_main, SpanWrapper};

use crate::context::RPLMetaContext;
use crate::error::{RPLMetaError, RPLMetaResult};
use crate::idx::RPLIdx;
use std::path::PathBuf;
use std::sync::Arc;

pub struct RPLMeta<'m> {
    /// Absolute path to the rpl file
    pub path: PathBuf,
    /// The name of the rpl file
    pub name: &'m str,
    /// The content of the rpl file
    pub content: &'m str,
}

impl<'m> RPLMeta<'m> {
    fn canonicalize(path: &PathBuf) -> RPLMetaResult<'m, PathBuf> {
        let res = std::fs::canonicalize(&path).map_err(|error| RPLMetaError::CanonicalizationError {
            error: Arc::new(error),
            path: path.to_path_buf(),
        })?;

        Ok(res)
    }

    /// Collect meta data from given string and path.
    pub fn collect_from_path(
        path: PathBuf,
        content: String,
        mctx: &'m mut RPLMetaContext<'m>,
    ) -> RPLMetaResult<'m, ()> {
        let path = Self::canonicalize(&path)?;
        let rpl_idx = mctx.request_rpl_idx(path.clone());
        mctx.contents.insert(rpl_idx, content.clone());

        // FIXME: consider multi-file/cache
        Self::parse_and_collect_all(content, path, rpl_idx, mctx)
    }

    pub fn parse_and_collect_all(
        content: String,
        path: PathBuf,
        idx: RPLIdx,
        mctx: &'m mut RPLMetaContext<'m>,
    ) -> RPLMetaResult<'m, ()> {
        let main = parse_main(content.as_str(), &path)?;
        Ok(())
    }
}
