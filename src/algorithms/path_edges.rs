use std::{hash::Hash, iter};

use petgraph::visit::{EdgeCount, EdgeRef, IntoEdgeReferences, IntoEdges, VisitMap, Visitable};

use crate::hash::HashSet;

#[derive(Clone, Copy)]
struct PathTreeNode<E> {
    edge: E,
    parent_index: Option<u32>,
}

pub struct TraversalSpace<G: Visitable> {
    graph: G,
    stack: Vec<(G::NodeId, Option<u32>)>,
    discovered: G::Map,
    path_tree: Vec<PathTreeNode<G::EdgeId>>,
}

impl<G: Visitable> TraversalSpace<G> {
    pub fn new(graph: G) -> Self
    where
        G::Map: Default,
    {
        Self {
            graph,
            stack: vec![],
            discovered: Default::default(),
            path_tree: vec![],
        }
    }
    fn reset(&mut self) {
        self.stack.clear();
        self.graph.reset_map(&mut self.discovered);
        self.path_tree.clear();
    }

    pub fn find_edges_in_cycles(&mut self) -> HashSet<G::EdgeId>
    where
        G: EdgeCount + IntoEdges + IntoEdgeReferences,
        G::EdgeId: Eq + Hash,
    {
        let mut edges = HashSet::<G::EdgeId>::default();
        edges.reserve(self.graph.edge_count());
        for edge_ref in self.graph.edge_references() {
            if edges.contains(&edge_ref.id()) {
                continue;
            }
            let Some(cycle_backtrack) =
                self.find_backtrack_edges(edge_ref.target(), edge_ref.source())
            else {
                continue;
            };
            edges.insert(edge_ref.id());
            edges.extend(cycle_backtrack);
        }
        edges
    }

    pub fn find_backtrack_edges(
        &mut self,
        from: G::NodeId,
        to: G::NodeId,
    ) -> Option<impl Iterator<Item = G::EdgeId> + '_>
    where
        G: IntoEdges,
    {
        self.reset();
        self.stack.push((from, None));

        while let Some((node, path_index)) = self.stack.pop() {
            if node == to {
                let mut path_index = path_index;
                let path_tree = self.path_tree.as_slice();
                return Some(iter::from_fn(move || {
                    if let Some(current_path_index) = path_index {
                        let path_tree_node = path_tree[current_path_index as usize];
                        path_index = path_tree_node.parent_index;
                        Some(path_tree_node.edge)
                    } else {
                        None
                    }
                }));
            }
            if self.discovered.visit(node) {
                for edge_ref in self.graph.edges(node) {
                    let neighbor = edge_ref.target();
                    if !self.discovered.is_visited(&neighbor) {
                        self.path_tree.push(PathTreeNode {
                            edge: edge_ref.id(),
                            parent_index: path_index,
                        });
                        self.stack
                            .push((neighbor, Some((self.path_tree.len() - 1) as u32)));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hashbrown::HashSet;
    use petgraph::Graph;

    #[test]
    fn test_find_backtrack_edges_disconnected() {
        let graph = Graph::<(), ()>::from_edges([(0, 1), (1, 2), (3, 4)]);
        let mut space = TraversalSpace::new(&graph);
        assert!(space.find_backtrack_edges(0.into(), 3.into()).is_none());
    }
    #[test]
    fn test_find_backtrack_edges_basic() {
        let graph = Graph::<(), ()>::from_edges([(0, 1), (1, 2), (2, 1), (2, 4), (4, 5)]);
        let mut space = TraversalSpace::new(&graph);
        let mut edges = space.find_backtrack_edges(0.into(), 4.into()).unwrap();
        assert_eq!(
            graph.edge_endpoints(edges.next().unwrap()),
            Some((2.into(), 4.into()))
        );
        assert_eq!(
            graph.edge_endpoints(edges.next().unwrap()),
            Some((1.into(), 2.into()))
        );
        assert_eq!(
            graph.edge_endpoints(edges.next().unwrap()),
            Some((0.into(), 1.into()))
        );
        assert!(edges.next().is_none());
    }
    #[test]
    fn test_find_edges_in_cycles_basic() {
        let graph = Graph::<(), ()>::from_edges([
            (0, 1),
            (1, 2),
            (2, 1),
            (2, 4),
            (4, 1),
            (4, 5),
            (5, 6),
            (6, 5),
        ]);
        let mut space = TraversalSpace::new(&graph);
        let edges = space.find_edges_in_cycles();
        let endpoints = edges
            .into_iter()
            .map(|edge_id| graph.edge_endpoints(edge_id).unwrap())
            .collect::<HashSet<_>>();
        assert_eq!(
            endpoints,
            [(1, 2), (2, 1), (2, 4), (4, 1), (5, 6), (6, 5)]
                .into_iter()
                .map(|(from, to)| (from.into(), to.into()))
                .collect()
        );
    }
}
