pub mod algorithms;
mod collect_deps;
mod dep_graph;
pub mod hash;
mod js_resolver;

pub use collect_deps::collect_dependencies;
pub use js_resolver::JsDiscoverDependency;
use oxc_resolver::{FileMetadata, FileSystem, ResolveOptions, ResolverGeneric};

use clap::Parser;
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

#[doc(hidden)]
pub fn run(args: &[&str], cwd: &Path) {}
