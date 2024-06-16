use std::{collections::VecDeque, ops::Deref, path::Path, sync::Arc};

use decycle::{collect_dependencies, JsDiscoverDependency, OsFileSystem};
use oxc_resolver::ResolveOptions;
use rustc_hash::FxHashSet;

fn main() {
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
        ["main.js"]
            .into_iter()
            .map(|path| Arc::from(Path::new(path))),
        &js_discover_dependency,
    );

    // dbg!(graph.errors_by_path);
    eprintln!("Finding cycles");

    let cycles = graph.dependency_graph.find_cycles();
    let mut n = 0;
    let mut cycle_set = FxHashSet::<VecDeque<&Path>>::default();
    for c in cycles {
        n += 1;
        let mut cycle = c.map(|path| path.deref()).collect::<VecDeque<&Path>>();
        move_min_to_first(&mut cycle);
        if !cycle_set.insert(cycle.clone()) {
            println!("repeat!! {:?}", cycle);
            break;
        }
        if n % 100000 == 0 || n % 100000 == 1 {
            println!("{n} {cycle:?}")
        }
    }
}

fn move_min_to_first<T: Ord>(deque: &mut VecDeque<T>) {
    let mut min_pos = 0;
    for i in 1..deque.len() {
        if deque[i] < deque[min_pos] {
            min_pos = i;
        }
    }
    deque.rotate_left(min_pos);
}

#[test]
fn test_move_min_to_first() {
    let mut d = VecDeque::from(vec![4, 3, 1, 2]);
    move_min_to_first(&mut d);
    assert_eq!(d.make_contiguous(), &[1, 2, 4, 3]);
}
