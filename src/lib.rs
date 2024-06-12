mod dep;

use std::path::Path;

use oxc_resolver::{FileSystem, ResolveOptions, ResolverGeneric};

use dep::get_deps;
use rayon::iter::{ParallelBridge, ParallelIterator};
use std::sync::mpsc;

pub fn scan<FS: FileSystem>(fs: FS, cwd: &str, entries: &[&str]) {
    let resolver = ResolverGeneric::<FS>::new_with_file_system(
        fs,
        ResolveOptions {
            extensions: [".jsx", ".js", ".tsx", ".ts"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            ..Default::default()
        },
    );

    let (deps_cx, deps_rx) = mpsc::channel::<(String, Vec<String>)>();
    let (work_cx, work_rx) = mpsc::channel::<(String, String)>();
    for entry in entries {
        work_cx.send((cwd.to_string(), entry.to_string())).unwrap();
    }
    rayon::join(
        || (),
        || {
            work_rx
                .into_iter()
                .par_bridge()
                .for_each(|(path, specifier)| {
                    let resolved_path = resolver.resolve(path, &specifier);
                })
        },
    );
}

#[cfg(test)]
mod tests {
    use oxc_resolver::Resolver;

    use super::*;

    #[test]
    fn it_works() {
        let resolver = Resolver::new(ResolveOptions {
            extensions: [".jsx", ".js", ".tsx", ".ts"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
            ..Default::default()
        });
        dbg!(resolver.resolve("/Users/patr0nus/code/hello_frontend", "./a"));
    }
}
