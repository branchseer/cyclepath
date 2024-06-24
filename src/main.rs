use std::{ops::Deref, path::Path, sync::Arc};

use decycle::{
    algorithms::johnson_simple_cycles::find_simple_cycles, algorithms::path_edges::TraversalSpace,
    collect_dependencies, hash::HashSet, JsDiscoverDependency, OsFileSystem,
};

use camino::{FromPathError, Utf8Path};
use oxc_resolver::ResolveOptions;

fn main() {
    let args = std::env::args().collect::<Vec<String>>();
    let entry = &args[1];
    let cwd = std::env::current_dir().unwrap();
    let js_discover_dependency = JsDiscoverDependency::new(
        OsFileSystem::default(),
        ResolveOptions {
            extensions: [".js", ".jsx", ".ts", ".tsx", ".node", ".json"]
                .into_iter()
                .map(String::from)
                .collect(),
            ..Default::default()
        },
    );
    eprintln!("Scanning");
    let graph = collect_dependencies(
        std::env::current_dir().unwrap().as_path(),
        [entry.as_str()]
            .into_iter()
            .map(|path| Arc::from(Path::new(path))),
        &js_discover_dependency,
    );

    let path_graph = graph.dependency_graph.path_graph();

    dbg!(path_graph.node_count(), path_graph.edge_count());
    dbg!(graph.errors_by_path);
    eprintln!("Finding cycle edges");

    let mut space = TraversalSpace::new(path_graph);
    let edges_in_cycles = space.find_edges_in_cycles();
    let mut endpoints = edges_in_cycles
        .into_iter()
        .map(|edge_id| -> Result<(&Utf8Path, &Utf8Path), FromPathError> {
            let (from_id, to_id) = path_graph.edge_endpoints(edge_id).unwrap();
            let from_path = path_graph[from_id].deref();
            let to_path = path_graph[to_id].deref();
            Ok((from_path.try_into()?, to_path.try_into()?))
        })
        .collect::<Result<Vec<_>, FromPathError>>()
        .unwrap();
    endpoints.sort_unstable();
    serde_json::to_writer_pretty(
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("./cyclepath-snapshot.json")
            .unwrap(),
        &endpoints,
    )
    .unwrap();
}
