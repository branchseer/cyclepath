use derive_where::derive_where;

use std::{ops::Deref, path::Path, sync::Arc};

use crate::{algorithms::johnson_simple_cycles::find_simple_cycles, hash::HashMap};
use petgraph::stable_graph::{NodeIndex, StableDiGraph};

#[derive(Debug)]
#[derive_where(Default)]
pub struct DependencyGraph<E> {
    path_graph: StableDiGraph<Arc<Path>, E>,
    node_indices_by_path: HashMap<Arc<Path>, NodeIndex>,
}

impl<E> DependencyGraph<E> {
    pub fn path_graph(&self) -> &StableDiGraph<Arc<Path>, E> {
        &self.path_graph
    }
    // pub fn node_indices_by_path(&self) -> &HashMap<Arc<Path>, NodeIndex> {
    //     &self.node_indices_by_path
    // }
    #[cfg(test)]
    pub fn paths<'a>(&'a self) -> impl Iterator<Item = &'a Path> {
        self.node_indices_by_path.keys().map(|p| p.deref())
    }

    #[cfg(test)]
    pub fn assert_consistency(&self) {
        assert_eq!(
            self.path_graph.node_count(),
            self.node_indices_by_path.len()
        );
        for (path, index) in &self.node_indices_by_path {
            assert_eq!(&self.path_graph[*index], path);
        }
    }

    #[cfg(test)]
    pub fn edges<'a>(&'a self) -> impl Iterator<Item = (&'a Path, &'a Path, &'a E)> {
        use petgraph::visit::{EdgeRef as _, IntoEdgeReferences as _};
        self.path_graph.edge_references().map(|edge_ref| {
            (
                self.path_graph[edge_ref.source()].deref(),
                self.path_graph[edge_ref.target()].deref(),
                edge_ref.weight(),
            )
        })
    }

    pub fn get_path_index_or_insert(&mut self, path: &Arc<Path>) -> (NodeIndex, bool) {
        let mut newly_inserted = false;
        let node_index = *self
            .node_indices_by_path
            .entry_ref(path.deref())
            .or_insert_with(|| {
                newly_inserted = true;
                self.path_graph.add_node(path.clone())
            });
        (node_index, newly_inserted)
    }
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, edge: E) {
        self.path_graph.add_edge(from, to, edge);
    }

    // To do: return edges (source span) along with paths
    pub fn find_cycles<'a>(&'a self) -> impl Iterator<Item = impl Iterator<Item = &'a Arc<Path>>> {
        let cycles = find_simple_cycles(&self.path_graph);
        cycles.map(|cycle| cycle.into_iter().map(|index| &self.path_graph[index]))
    }
}
