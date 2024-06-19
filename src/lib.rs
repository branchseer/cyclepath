mod algorithms;
mod collect_deps;
mod dep_graph;
mod hash;
mod js_resolver;

pub use collect_deps::collect_dependencies;
pub use js_resolver::JsDiscoverDependency;
use oxc_resolver::{FileMetadata, FileSystem, ResolveOptions, ResolverGeneric};

use rayon::iter::{ParallelBridge, ParallelIterator};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{mpsc, Arc},
};

#[derive(Default, Clone, Debug)]
pub struct OsFileSystem(());

impl FileSystem for OsFileSystem {
    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        std::fs::read_to_string(path)
    }

    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        std::fs::metadata(path).map(FileMetadata::from)
    }

    fn symlink_metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        std::fs::symlink_metadata(path).map(FileMetadata::from)
    }

    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        dunce::canonicalize(path)
    }
}

pub fn get_circles<FS: FileSystem + Clone>(fs: FS, entries: impl Iterator<Item = Arc<Path>>) {
    let js_discover_dependency = JsDiscoverDependency::new(
        fs,
        ResolveOptions {
            extensions: [".js", ".jsx", ".ts", ".tsx", ".node"]
                .into_iter()
                .map(String::from)
                .collect(),
            ..Default::default()
        },
    );
    let result = collect_dependencies(entries, &js_discover_dependency);
}
