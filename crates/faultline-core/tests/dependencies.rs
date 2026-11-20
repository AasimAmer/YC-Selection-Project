use faultline_contract::{
    FileId, GraphNode, PartialResultReason, SourceLocation, SourceRange, SymbolId, SymbolKind,
};
use faultline_core::{DependencyGraph, GraphError, symbol_id};

fn id(name: &str) -> SymbolId {
    SymbolId(name.into())
}

fn graph(names: &[&str], relations: &[(&str, &str)]) -> DependencyGraph {
    let mut graph = DependencyGraph::default();
    for name in names {
        graph
            .insert_node(GraphNode {
                id: id(name),
                name: (*name).into(),
                kind: SymbolKind::Function,
                range: SourceRange {
                    repo_relative_file: FileId("main.ts".into()),
                    start: SourceLocation { line: 1, byte: 0 },
                    end: SourceLocation { line: 1, byte: 4 },
                },
            })
            .unwrap();
    }
    for &(consumer, dependency) in relations {
        graph
            .insert_relation(&id(consumer), &id(dependency))
            .unwrap();
    }
    graph
}

#[test]
fn reverse_traversal_follows_consumers_even_with_cycles_and_diamonds() {
    let graph = graph(
        &["a", "b", "c", "d"],
        &[
            ("c", "a"),
            ("b", "a"),
            ("b", "a"),
            ("a", "a"),
            ("d", "c"),
            ("d", "b"),
            ("a", "d"),
        ],
    );
    let forward = graph
        .dependencies(&id("d"))
        .unwrap()
        .map(|node| node.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(forward, ["b", "c"]);
    let traversal = graph.reverse(&id("a"), 2, 3).unwrap();
    let paths = traversal
        .visits
        .iter()
        .map(|visit| {
            (
                visit.consumer.name.as_str(),
                visit.dependency.0.as_str(),
                visit.hops,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(paths, [("b", "a", 1), ("c", "a", 1), ("d", "b", 2)]);
    assert_eq!(traversal.partial_reason, None);
}

#[test]
fn traversal_reports_limits_without_partial_results_for_isolated_nodes() {
    let mut graph = graph(&["a", "b", "c", "unused"], &[("b", "a"), ("c", "b")]);
    let target = id("a");
    assert_eq!(
        graph.reverse(&target, 1, 10).unwrap().partial_reason,
        Some(PartialResultReason::HopLimit)
    );
    let capped = graph.reverse(&target, 10, 1).unwrap();
    assert_eq!(capped.visits.len(), 1);
    assert_eq!(capped.partial_reason, Some(PartialResultReason::NodeBudget));
    assert_eq!(
        graph.reverse(&id("unused"), 0, 0).unwrap().partial_reason,
        None
    );
    assert_eq!(
        graph.insert_relation(&id("unused"), &id("missing")),
        Err(GraphError::UnknownNode)
    );
    assert_eq!(graph.dependencies(&id("unused")).unwrap().count(), 0);
    assert_eq!(
        graph.reverse(&id("missing"), 10, 10),
        Err(GraphError::UnknownNode)
    );
}

#[test]
fn logical_ids_distinguish_kinds_paths_nesting_and_siblings() {
    let file = FileId("src/lib.ts".into());
    let kind = SymbolKind::Function;
    let original = symbol_id(&file, &["outer", "inner"], &kind, 0);
    assert_eq!(original, symbol_id(&file, &["outer", "inner"], &kind, 0));
    assert_ne!(original, symbol_id(&file, &["outer::inner"], &kind, 0));
    assert_ne!(
        original,
        symbol_id(&file, &["outer", "inner"], &SymbolKind::Method, 0)
    );
    assert_ne!(original, symbol_id(&file, &["outer", "inner"], &kind, 1));
    assert_ne!(
        original,
        symbol_id(&FileId("lib.ts".into()), &["outer", "inner"], &kind, 0)
    );
}
