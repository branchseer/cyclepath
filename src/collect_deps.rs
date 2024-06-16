use std::{
    path::Path,
    sync::{mpsc, Arc},
};

use crate::dep_graph::DependencyGraph;
use crate::hash::HashMap;

use rayon::iter::{ParallelBridge, ParallelIterator};

pub trait DiscoverDependency: Send + Sync {
    type Edge: Send;
    type Error: Send;
    fn discover_dependencies(
        &self,
        path: &Path,
    ) -> (Vec<(Arc<Path>, Self::Edge)>, Option<Self::Error>);
}

struct DependencyInfo<Edge, Error> {
    path: Arc<Path>,
    dependencies: Vec<(Arc<Path>, Edge)>,
    error: Option<Error>,
}

#[derive(Debug)]
pub struct DependencyGraphWithErrors<Edge, Error> {
    pub dependency_graph: DependencyGraph<Edge>,
    pub errors_by_path: HashMap<Arc<Path>, Error>,
}

pub fn collect_dependencies<D: DiscoverDependency>(
    paths: impl Iterator<Item = Arc<Path>>,
    dep_discoverer: &D,
) -> DependencyGraphWithErrors<D::Edge, D::Error> {
    let (deps_cx, deps_rx) = mpsc::channel::<DependencyInfo<D::Edge, D::Error>>();
    let (work_cx, work_rx) = mpsc::channel::<Arc<Path>>();

    let mut remaining = 0u32;
    for path in paths {
        work_cx.send(path).unwrap();
        remaining += 1;
    }

    let (_, dep_graph) = rayon::join(
        move || {
            work_rx.into_iter().par_bridge().for_each(move |path| {
                let (dependencies, error) = dep_discoverer.discover_dependencies(&path);
                deps_cx
                    .send(DependencyInfo {
                        path,
                        dependencies,
                        error,
                    })
                    .unwrap();
            })
        },
        move || {
            let mut dep_graph = DependencyGraph::<D::Edge>::default();
            let mut errors_by_path = HashMap::<Arc<Path>, D::Error>::default();
            for DependencyInfo {
                path,
                dependencies,
                error,
            } in deps_rx
            {
                remaining = remaining.checked_sub(1).unwrap();
                let (from_index, _) = dep_graph.get_path_index_or_insert(&path);
                for (dep_path, edge) in dependencies {
                    let (to_index, newly_inserted) = dep_graph.get_path_index_or_insert(&dep_path);
                    if newly_inserted {
                        remaining = remaining.checked_add(1).unwrap();
                        work_cx.send(dep_path).unwrap()
                    }
                    dep_graph.add_edge(from_index, to_index, edge);
                }
                if let Some(error) = error {
                    assert!(errors_by_path.insert(path, error).is_none());
                }
                if remaining == 0 {
                    break;
                }
            }
            DependencyGraphWithErrors {
                dependency_graph: dep_graph,
                errors_by_path,
            }
        },
    );
    dep_graph
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::hash::{HashMap, HashSet};
    struct TestDiscoverDependency(
        HashMap<&'static Path, (Vec<(&'static Path, &'static str)>, Option<&'static str>)>,
    );

    impl DiscoverDependency for TestDiscoverDependency {
        type Edge = &'static str;
        type Error = &'static str;

        fn discover_dependencies(
            &self,
            path: &Path,
        ) -> (Vec<(Arc<Path>, Self::Edge)>, Option<Self::Error>) {
            let (deps, err) = &self.0[path];
            (
                deps.into_iter()
                    .map(|(dep_path, edge)| (Arc::from(*dep_path), *edge))
                    .collect(),
                *err,
            )
        }
    }
    fn ap(path_str: &'static str) -> Arc<Path> {
        Path::new(path_str).into()
    }
    fn p(path_str: &'static str) -> &'static Path {
        Path::new(path_str)
    }
    #[test]
    fn test_collect_dependencies() {
        let test_discover_dep = TestDiscoverDependency({
            let mut map = HashMap::default();
            map.insert(p(""), (vec![], None));
            map.insert(p("a"), (vec![(p("b"), "a-b")], Some("a error")));
            map.insert(p("b"), (vec![(p("c"), "b-c"), (p("d"), "b-d")], None));
            map.insert(p("c"), (vec![], Some("c error")));
            map.insert(p("d"), (vec![(p("a"), "d-a"), (p("d"), "d-d")], None));
            map
        });
        let result = collect_dependencies([ap(""), ap("a")].into_iter(), &test_discover_dep);

        assert_eq!(result.errors_by_path[p("a")], "a error");
        assert_eq!(result.errors_by_path[p("c")], "c error");
        assert_eq!(result.errors_by_path.len(), 2);

        result.dependency_graph.assert_consistency();

        let actual_paths = result.dependency_graph.paths().collect::<HashSet<_>>();
        assert_eq!(
            actual_paths,
            [p(""), p("a"), p("b"), p("c"), p("d")]
                .into_iter()
                .collect()
        );

        let actual_edges = result.dependency_graph.edges().collect::<HashSet<_>>();
        assert_eq!(
            actual_edges,
            [
                (p("a"), p("b"), &"a-b"),
                (p("b"), p("c"), &"b-c"),
                (p("b"), p("d"), &"b-d"),
                (p("d"), p("a"), &"d-a"),
                (p("d"), p("d"), &"d-d"),
            ]
            .into_iter()
            .collect()
        )
    }
}
