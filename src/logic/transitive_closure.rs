//! Pure transitive closure computation for directed graphs.
//!
//! Used to pre-compute `bibitem_further_refs` from the direct `bibitem_refs` edges.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;

/// Compute the transitive closure of a directed graph.
///
/// Given edges as `(source, dep)` pairs, returns all `(source, dep)` pairs
/// reachable transitively. Self-loops (`source == dep`) are excluded from output.
///
/// Handles cycles correctly: BFS per source with a visited set ensures each
/// `(source, dep)` pair is produced exactly once regardless of cycles.
pub fn transitive_closure<T>(edges: &[(T, T)]) -> Vec<(T, T)>
where
    T: Hash + Eq + Clone,
{
    if edges.is_empty() {
        return Vec::new();
    }

    let mut adj: HashMap<T, Vec<T>> = HashMap::new();
    for (src, dep) in edges {
        adj.entry(src.clone()).or_default().push(dep.clone());
    }

    let mut result = Vec::new();

    for source in adj.keys().cloned().collect::<Vec<_>>() {
        let mut reachable: HashSet<T> = HashSet::new();
        let mut queue: VecDeque<T> = VecDeque::new();

        for dep in &adj[&source] {
            if *dep != source && reachable.insert(dep.clone()) {
                queue.push_back(dep.clone());
            }
        }

        while let Some(current) = queue.pop_front() {
            if let Some(next_deps) = adj.get(&current) {
                for dep in next_deps {
                    if *dep != source && reachable.insert(dep.clone()) {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }

        for dep in reachable {
            result.push((source.clone(), dep));
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted(mut pairs: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
        pairs.sort_unstable();
        pairs
    }

    #[test]
    fn empty_graph() {
        assert_eq!(transitive_closure::<i64>(&[]), vec![]);
    }

    #[test]
    fn single_edge() {
        let result = sorted(transitive_closure(&[(1, 2)]));
        assert_eq!(result, vec![(1, 2)]);
    }

    #[test]
    fn chain_a_b_c() {
        // A→B→C: A reaches B and C; B reaches C
        let edges = [(1, 2), (2, 3)];
        let result = sorted(transitive_closure(&edges));
        assert_eq!(result, vec![(1, 2), (1, 3), (2, 3)]);
    }

    #[test]
    fn self_loop_excluded() {
        // A→A: no output (self-loop excluded)
        assert_eq!(transitive_closure(&[(1, 1)]), vec![]);
    }

    #[test]
    fn self_loop_with_other_edge() {
        // A→A, A→B: A reaches B (not A itself)
        let result = sorted(transitive_closure(&[(1, 1), (1, 2)]));
        assert_eq!(result, vec![(1, 2)]);
    }

    #[test]
    fn direct_cycle() {
        // A→B→A: A reaches B, B reaches A (not themselves)
        let edges = [(1, 2), (2, 1)];
        let result = sorted(transitive_closure(&edges));
        assert_eq!(result, vec![(1, 2), (2, 1)]);
    }

    #[test]
    fn longer_cycle() {
        // A→B→C→A: each node reaches the other two, not itself
        let edges = [(1, 2), (2, 3), (3, 1)];
        let result = sorted(transitive_closure(&edges));
        assert_eq!(result, vec![(1, 2), (1, 3), (2, 1), (2, 3), (3, 1), (3, 2)]);
    }

    #[test]
    fn cycle_with_tail() {
        // A→B→C, C→D (no cycle), D→B (back-edge into cycle)
        // A reaches: B, C, D
        // B reaches: C, D (D→B creates cycle between B,C,D but B≠B excluded)
        // C reaches: D, B
        // D reaches: B, C
        let edges = [(1, 2), (2, 3), (3, 4), (4, 2)];
        let result = sorted(transitive_closure(&edges));
        let expected = sorted(vec![
            (1, 2),
            (1, 3),
            (1, 4), // A reaches B, C, D
            (2, 3),
            (2, 4), // B reaches C, D (not B itself via D→B)
            (3, 4),
            (3, 2), // C reaches D, B
            (4, 2),
            (4, 3), // D reaches B, C
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn disconnected_components() {
        // A→B and C→D are independent
        let edges = [(1, 2), (3, 4)];
        let result = sorted(transitive_closure(&edges));
        assert_eq!(result, vec![(1, 2), (3, 4)]);
    }

    #[test]
    fn diamond() {
        // A→B, A→C, B→D, C→D
        // A reaches B, C, D; B reaches D; C reaches D
        let edges = [(1, 2), (1, 3), (2, 4), (3, 4)];
        let result = sorted(transitive_closure(&edges));
        let expected = sorted(vec![(1, 2), (1, 3), (1, 4), (2, 4), (3, 4)]);
        assert_eq!(result, expected);
    }

    #[test]
    fn multiple_sources_same_target() {
        // A→C, B→C: independent sources, same target
        let edges = [(1, 3), (2, 3)];
        let result = sorted(transitive_closure(&edges));
        assert_eq!(result, vec![(1, 3), (2, 3)]);
    }

    #[test]
    fn no_revisit_in_long_chain() {
        // A→B→C→D→E: A reaches all four, no duplicate pairs
        let edges = [(1, 2), (2, 3), (3, 4), (4, 5)];
        let result = sorted(transitive_closure(&edges));
        let expected = sorted(vec![
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (2, 3),
            (2, 4),
            (2, 5),
            (3, 4),
            (3, 5),
            (4, 5),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn node_only_as_target_not_in_output_as_source() {
        // A→B, B has no outgoing edges; only (A,B) expected
        let edges = [(1, 2)];
        let result = sorted(transitive_closure(&edges));
        assert_eq!(result, vec![(1, 2)]);
    }

    #[test]
    fn cycle_with_exit_to_acyclic_tail() {
        // A→B→C→A (cycle) + C→D→E (acyclic tail exiting the cycle)
        // A reaches B, C, D, E (enters cycle then follows tail)
        // B reaches C, A, D, E
        // C reaches A, B, D, E
        // D reaches E only
        let edges = [(1, 2), (2, 3), (3, 1), (3, 4), (4, 5)];
        let result = sorted(transitive_closure(&edges));
        let expected = sorted(vec![
            (1, 2),
            (1, 3),
            (1, 4),
            (1, 5),
            (2, 3),
            (2, 1),
            (2, 4),
            (2, 5),
            (3, 1),
            (3, 2),
            (3, 4),
            (3, 5),
            (4, 5),
        ]);
        assert_eq!(result, expected);
    }

    #[test]
    fn duplicate_input_edges_no_duplicate_output() {
        // Same edge twice in input — output should still have it once
        let edges = [(1, 2), (1, 2), (2, 3)];
        let result = sorted(transitive_closure(&edges));
        assert_eq!(result, vec![(1, 2), (1, 3), (2, 3)]);
    }
}
