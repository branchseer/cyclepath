// Licensed under the apache license, version 2.0 (the "license"); you may
// not use this file except in compliance with the License. You may obtain
// a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the
// License for the specific language governing permissions and limitations
// under the License.

// https://github.com/Qiskit/rustworkx/blob/9f0646e8886cfecc55e59b96532c6f7f798524c0/src/connectivity/johnson_simple_cycles.rs

use std::hash::Hash;

use ahash::RandomState;
use hashbrown::{HashMap, HashSet};
use indexmap::IndexSet;

use petgraph::algo::kosaraju_scc;
use petgraph::graph::IndexType;
use petgraph::graph::NodeIndex;
use petgraph::stable_graph::StableDiGraph;
use petgraph::visit::EdgeCount;
use petgraph::visit::EdgeRef;
use petgraph::visit::GraphBase;
use petgraph::visit::IntoEdgeReferences;
use petgraph::visit::IntoNeighborsDirected;
use petgraph::visit::IntoNodeIdentifiers;
use petgraph::visit::NodeFiltered;
use petgraph::visit::Visitable;

fn build_subgraph<G: EdgeCount + IntoNodeIdentifiers + IntoEdgeReferences>(
    graph: G,
    nodes: &[G::NodeId],
) -> (StableDiGraph<(), ()>, HashMap<G::NodeId, NodeIndex>)
where
    G::NodeId: Hash + Eq,
{
    let node_set: HashSet<G::NodeId> = nodes.iter().copied().collect();
    let mut node_map: HashMap<G::NodeId, NodeIndex> = HashMap::with_capacity(nodes.len());
    let node_filter = |node: G::NodeId| -> bool { node_set.contains(&node) };
    // Overallocates edges, but not a big deal as this is temporary for the lifetime of the
    // subgraph
    let mut out_graph = StableDiGraph::<(), ()>::with_capacity(nodes.len(), graph.edge_count());
    let filtered = NodeFiltered(&graph, node_filter);
    for node_id in filtered.node_identifiers() {
        let new_node = out_graph.add_node(());
        node_map.insert(node_id, new_node);
    }
    for edge in filtered.edge_references() {
        let new_source = *node_map.get(&edge.source()).unwrap();
        let new_target = *node_map.get(&edge.target()).unwrap();
        out_graph.add_edge(new_source, new_target, ());
    }
    (out_graph, node_map)
}

fn unblock(
    node: NodeIndex,
    blocked: &mut HashSet<NodeIndex>,
    block: &mut HashMap<NodeIndex, HashSet<NodeIndex>>,
) {
    let mut stack: IndexSet<NodeIndex, RandomState> = IndexSet::with_hasher(RandomState::default());
    stack.insert(node);
    while let Some(stack_node) = stack.pop() {
        if blocked.remove(&stack_node) {
            match block.get_mut(&stack_node) {
                // stack.update(block[stack_node]):
                Some(block_set) => {
                    block_set.drain().for_each(|n| {
                        stack.insert(n);
                    });
                }
                // If block doesn't have stack_node treat it as an empty set
                // (so no updates to stack) and populate it with an empty
                // set.
                None => {
                    block.insert(stack_node, HashSet::new());
                }
            }
            blocked.remove(&stack_node);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_stack<G: GraphBase>(
    start_node: NodeIndex,
    stack: &mut Vec<(NodeIndex, IndexSet<NodeIndex, RandomState>)>,
    path: &mut Vec<NodeIndex>,
    closed: &mut HashSet<NodeIndex>,
    blocked: &mut HashSet<NodeIndex>,
    block: &mut HashMap<NodeIndex, HashSet<NodeIndex>>,
    subgraph: &StableDiGraph<(), ()>,
    reverse_node_map: &HashMap<NodeIndex, G::NodeId>,
) -> Option<Vec<G::NodeId>> {
    while let Some((this_node, neighbors)) = stack.last_mut() {
        if let Some(next_node) = neighbors.pop() {
            if next_node == start_node {
                // Out path in input graph basis
                let mut out_path: Vec<G::NodeId> = Vec::with_capacity(path.len());
                for n in path {
                    out_path.push(reverse_node_map[n]);
                    closed.insert(*n);
                }
                return Some(out_path);
            } else if blocked.insert(next_node) {
                path.push(next_node);
                stack.push((
                    next_node,
                    subgraph
                        .neighbors(next_node)
                        .collect::<IndexSet<NodeIndex, ahash::RandomState>>(),
                ));
                closed.remove(&next_node);
                blocked.insert(next_node);
                continue;
            }
        }
        if neighbors.is_empty() {
            if closed.contains(this_node) {
                unblock(*this_node, blocked, block);
            } else {
                for neighbor in subgraph.neighbors(*this_node) {
                    let block_neighbor = block.entry(neighbor).or_insert_with(HashSet::new);
                    block_neighbor.insert(*this_node);
                }
            }
            stack.pop();
            path.pop();
        }
    }
    None
}

pub struct SimpleCycleIter<G: GraphBase> {
    graph: G,
    scc: Vec<Vec<G::NodeId>>,
    path: Vec<NodeIndex>,
    blocked: HashSet<NodeIndex>,
    closed: HashSet<NodeIndex>,
    block: HashMap<NodeIndex, HashSet<NodeIndex>>,
    stack: Vec<(NodeIndex, IndexSet<NodeIndex, RandomState>)>,
    start_node: NodeIndex,
    node_map: HashMap<G::NodeId, NodeIndex>,
    reverse_node_map: HashMap<NodeIndex, G::NodeId>,
    subgraph: StableDiGraph<(), ()>,
}

pub fn find_simple_cycles<G: GraphBase>(graph: G) -> SimpleCycleIter<G>
where
    G::NodeId: IndexType,
    for<'a> &'a G: IntoNodeIdentifiers<NodeId = G::NodeId>
        + IntoNeighborsDirected
        + Visitable
        + EdgeCount
        + IntoEdgeReferences,
{
    let strongly_connected_components: Vec<Vec<G::NodeId>> =
        kosaraju_scc(&graph).into_iter().collect();
    SimpleCycleIter {
        graph,
        scc: strongly_connected_components,
        path: Vec::new(),
        blocked: HashSet::new(),
        closed: HashSet::new(),
        block: HashMap::new(),
        stack: Vec::new(),
        start_node: NodeIndex::new(std::u32::MAX as usize),
        node_map: HashMap::new(),
        reverse_node_map: HashMap::new(),
        subgraph: StableDiGraph::new(),
    }
}

impl<G: GraphBase> Iterator for SimpleCycleIter<G>
where
    G::NodeId: IndexType,
    for<'a> &'a G: IntoNodeIdentifiers<NodeId = G::NodeId>
        + IntoNeighborsDirected
        + Visitable
        + EdgeCount
        + IntoEdgeReferences,
{
    type Item = Vec<G::NodeId>;

    fn next(&mut self) -> Option<Self::Item> {
        // Restore previous state if it exists
        let mut stack: Vec<(NodeIndex, IndexSet<NodeIndex, ahash::RandomState>)> =
            std::mem::take(&mut self.stack);
        let mut path: Vec<NodeIndex> = std::mem::take(&mut self.path);
        let mut closed: HashSet<NodeIndex> = std::mem::take(&mut self.closed);
        let mut blocked: HashSet<NodeIndex> = std::mem::take(&mut self.blocked);
        let mut block: HashMap<NodeIndex, HashSet<NodeIndex>> = std::mem::take(&mut self.block);
        let mut subgraph: StableDiGraph<(), ()> = std::mem::take(&mut self.subgraph);
        let mut reverse_node_map: HashMap<NodeIndex, G::NodeId> =
            std::mem::take(&mut self.reverse_node_map);
        let mut node_map: HashMap<G::NodeId, NodeIndex> = std::mem::take(&mut self.node_map);

        if let Some(res) = process_stack::<G>(
            self.start_node,
            &mut stack,
            &mut path,
            &mut closed,
            &mut blocked,
            &mut block,
            &subgraph,
            &reverse_node_map,
        ) {
            // Store internal state on yield
            self.stack = stack;
            self.path = path;
            self.closed = closed;
            self.blocked = blocked;
            self.block = block;
            self.subgraph = subgraph;
            self.reverse_node_map = reverse_node_map;
            self.node_map = node_map;
            return Some(res);
        } else {
            subgraph.remove_node(self.start_node);
            self.scc
                .extend(kosaraju_scc(&subgraph).into_iter().filter_map(|scc| {
                    let res = scc
                        .iter()
                        .map(|n| reverse_node_map[n])
                        .collect::<Vec<G::NodeId>>();
                    Some(res)
                }));
        }
        while let Some(mut scc) = self.scc.pop() {
            let temp = build_subgraph(&self.graph, &scc);
            subgraph = temp.0;
            node_map = temp.1;
            reverse_node_map = node_map.iter().map(|(k, v)| (*v, *k)).collect();
            // start_node, path, blocked, closed, block and stack all in subgraph basis
            self.start_node = node_map[&scc.pop().unwrap()];
            path = vec![self.start_node];
            blocked = path.iter().copied().collect();
            // Nodes in cycle all
            closed = HashSet::new();
            block = HashMap::new();
            stack = vec![(
                self.start_node,
                subgraph
                    .neighbors(self.start_node)
                    .collect::<IndexSet<NodeIndex, ahash::RandomState>>(),
            )];
            if let Some(res) = process_stack::<G>(
                self.start_node,
                &mut stack,
                &mut path,
                &mut closed,
                &mut blocked,
                &mut block,
                &subgraph,
                &reverse_node_map,
            ) {
                // Store internal state on yield
                self.stack = stack;
                self.path = path;
                self.closed = closed;
                self.blocked = blocked;
                self.block = block;
                self.subgraph = subgraph;
                self.reverse_node_map = reverse_node_map;
                self.node_map = node_map;
                return Some(res);
            }
            subgraph.remove_node(self.start_node);
            self.scc
                .extend(kosaraju_scc(&subgraph).into_iter().map(|scc| {
                    scc.iter()
                        .map(|n| reverse_node_map[n])
                        .collect::<Vec<G::NodeId>>()
                }));
        }
        None
    }
}

#[cfg(test)]
mod test_johnson_simple_cycles {
    use super::*;
    use petgraph::Graph;
    use rustworkx_core::generators::complete_graph;
    use test_case::{test_case, test_matrix};

    fn collect_simple_cycles(graph: &Graph<(), ()>) -> Vec<Vec<usize>> {
        let mut cycles = find_simple_cycles(graph)
            .map(|nodes| {
                let mut nodes = nodes
                    .into_iter()
                    .map(NodeIndex::index)
                    .collect::<Vec<usize>>();
                nodes.sort_unstable();
                nodes
            })
            .collect::<Vec<_>>();
        cycles.sort_unstable();
        cycles
    }

    #[test]
    fn test_simple_cycles() {
        let mut graph = Graph::<(), ()>::new();
        graph.extend_with_edges([(0, 0), (0, 1), (0, 2), (1, 2), (2, 0), (2, 1), (2, 2)]);
        let expected: &[&[usize]] = &[&[0], &[0, 1, 2], &[0, 2], &[1, 2], &[2]];
        let actual = collect_simple_cycles(&graph);
        assert_eq!(expected, actual);
    }

    /*
        Test taken from Table 2 in the Johnson Algorithm paper
        which shows the number of cycles in a complete graph of
        2 to 9 nodes and the time to calculate it on a s370/168
        The table in question is a benchmark comparing the runtime
        to tarjan's algorithm, but it gives us a good test with
        a known value (networkX does this too)
    */
    #[test_case(2, 1)]
    #[test_case(3, 5)]
    #[test_case(4, 20)]
    #[test_case(5, 84)]
    #[test_case(6, 409)]
    #[test_case(7, 2365)]
    #[test_case(8, 16064)]
    fn test_mesh_graph(node_count: usize, expected_cycle_count: usize) {
        let graph: Graph<(), ()> = complete_graph(Some(node_count), None, || (), || ()).unwrap();
        let cycles = find_simple_cycles(graph);
        assert_eq!(cycles.count(), expected_cycle_count);
    }

    #[test]
    fn test_empty_graph() {
        let empty_graph = Graph::<(), ()>::default();
        assert_eq!(find_simple_cycles(empty_graph).count(), 0);
    }

    // This graph tests figured 1 from the Johnson's algorithm paper
    #[test_matrix(3..10)]
    fn test_figure_1(k: u32) {
        let mut graph = Graph::<(), ()>::new();
        let mut edge_list = Vec::<(u32, u32)>::new();
        for n in 2..(k + 2) {
            edge_list.push((1, n));
            edge_list.push((n, k + 2));
        }
        edge_list.push((2 * k + 1, 1));
        for n in (k + 2)..(2 * k + 2) {
            edge_list.push((n, 2 * k + 2));
            edge_list.push((n, n + 1));
        }
        edge_list.push((2 * k + 3, k + 2));
        for n in (2 * k + 3)..(3 * k + 3) {
            edge_list.push((2 * k + 2, n));
            edge_list.push((n, 3 * k + 3));
        }
        edge_list.push((3 * k + 3, 2 * k + 2));
        graph.extend_with_edges(edge_list);
        let cycles = find_simple_cycles(graph);
        assert_eq!(cycles.count(), 3 * k as usize);
    }
}
