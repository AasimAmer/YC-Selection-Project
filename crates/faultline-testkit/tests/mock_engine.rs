use std::{fs, path::Path, sync::Arc, thread};

use faultline_contract::*;
use faultline_testkit::{MockEngine, Scenario};

const SESSION: &str = "src/session.ts::validateSession";
const AUDIT: &str = "src/audit.ts::auditSession";
const MIDDLEWARE: &str = "src/middleware.ts::authorizeRequest";
const ROUTER: &str = "src/router.ts::routeRequest";
const NORMALIZE: &str = "src/cycle/normalize.js::normalizeToken";
const PREFIX: &str = "src/cycle/prefix.js::stripPrefix";

fn search(query: &str) -> SearchRequest {
    SearchRequest {
        query: query.into(),
        limit: 10,
        expected_snapshot: None,
    }
}

fn references(target: &str) -> ReferencesRequest {
    ReferencesRequest {
        target: SymbolId(target.into()),
        limit: 10,
        expected_snapshot: None,
    }
}

fn impact(target: &str) -> ImpactRequest {
    ImpactRequest {
        target: SymbolId(target.into()),
        max_hops: 3,
        node_budget: 10,
        deadline_ms: None,
        limit: 10,
        expected_snapshot: None,
    }
}

fn graph(target: &str) -> GraphRequest {
    GraphRequest {
        target: SymbolId(target.into()),
        max_hops: 3,
        node_budget: 10,
        limit: 10,
        expected_snapshot: None,
    }
}

fn assert_partial(meta: &ResponseMeta, reason: PartialResultReason) {
    assert!(meta.truncated);
    assert_eq!(meta.partial_reason, Some(reason));
}

fn symbol_ids(nodes: &[GraphNode]) -> Vec<&str> {
    nodes.iter().map(|node| node.id.0.as_str()).collect()
}

fn source_snippet(range: &SourceRange) -> String {
    let demo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/demo");
    let path = &range.repo_relative_file.0;
    let source = fs::read_to_string(demo.join("repo").join(path))
        .or_else(|_| fs::read_to_string(demo.join("edge-cases/repo").join(path)))
        .unwrap_or_else(|error| panic!("{path}: {error}"));
    let start = usize::try_from(range.start.byte).unwrap();
    let end = usize::try_from(range.end.byte).unwrap();
    assert!(start < end, "{range:?}");
    let line = |offset| {
        source[..offset]
            .bytes()
            .filter(|&byte| byte == b'\n')
            .count()
            + 1
    };
    assert_eq!(line(start), range.start.line as usize, "{range:?}");
    assert_eq!(line(end), range.end.line as usize, "{range:?}");
    source[start..end].into()
}

#[test]
fn health_counts_match_the_demo_and_empty_snapshots() {
    let engine = MockEngine::default();
    let health = engine.health().unwrap();
    let graph = engine.graph(graph(ROUTER)).unwrap();
    let files: std::collections::BTreeSet<_> = graph
        .nodes
        .iter()
        .map(|node| &node.range.repo_relative_file.0)
        .collect();
    assert!(!files.is_empty());
    assert_eq!(health.meta.freshness, Freshness::Current);
    assert_eq!(health.files, files.len() as u64);
    assert_eq!(health.symbols, graph.nodes.len() as u64);
    assert_eq!(health.resolved_edges, graph.edges.len() as u64);

    let empty = MockEngine::empty().health().unwrap();
    assert_eq!(empty.meta.freshness, Freshness::Current);
    assert_eq!(
        (empty.files, empty.symbols, empty.resolved_edges),
        (0, 0, 0)
    );

    let stale = MockEngine::stale().health().unwrap();
    assert_eq!(stale.meta.freshness, Freshness::Stale);
    assert_eq!(stale.meta.snapshot_epoch, health.meta.snapshot_epoch);
    assert_eq!(
        (stale.files, stale.symbols, stale.resolved_edges),
        (health.files, health.symbols, health.resolved_edges)
    );
    assert_eq!(
        MockEngine::error(EngineError::Unavailable).health(),
        Err(EngineError::Unavailable)
    );
}

#[test]
fn search_matches_names_and_paths_and_limits_ranked_results() {
    let engine = MockEngine::default();
    let exact = engine.search(search("  VALIDATESESSION  ")).unwrap();
    assert_eq!(symbol_ids(&exact.results), [SESSION]);
    assert!(
        source_snippet(&exact.results[0].range).starts_with("export function validateSession(")
    );
    assert_eq!(
        symbol_ids(&engine.search(search("cycle/")).unwrap().results),
        [NORMALIZE, PREFIX]
    );
    let results = engine.search(search("session")).unwrap();
    assert_eq!(symbol_ids(&results.results), [AUDIT, SESSION]);
    for bound in [0, 1, 2, u32::MAX] {
        let response = engine
            .search(SearchRequest {
                limit: bound,
                ..search("session")
            })
            .unwrap();
        let len = (bound as usize).min(results.results.len());
        assert_eq!(response.results, results.results[..len]);
        if len < results.results.len() {
            assert_partial(&response.meta, PartialResultReason::ResultLimit);
        } else {
            assert_eq!(response.meta, results.meta);
        }
    }
    let unknown = engine.search(search("unknown")).unwrap();
    assert!(unknown.results.is_empty());
    assert!(!unknown.meta.truncated);
}

#[test]
fn references_are_incoming_edges_with_byte_accurate_source_evidence() {
    let engine = MockEngine::default();
    let result = engine.references(references(SESSION)).unwrap();
    assert_eq!(
        result,
        engine.references(references("validateSession")).unwrap()
    );
    assert_eq!(result.results.len(), 2);
    for (edge, (consumer, file)) in result
        .results
        .iter()
        .zip([(AUDIT, "src/audit.ts"), (MIDDLEWARE, "src/middleware.ts")])
    {
        assert_eq!(edge.consumer.0, consumer);
        assert_eq!(edge.dependency, Some(SymbolId(SESSION.into())));
        assert_eq!(edge.kind, EdgeKind::Calls);
        assert_eq!(edge.resolution, ResolutionState::Resolved);
        assert_eq!(edge.range.repo_relative_file.0, file);
        assert_eq!(source_snippet(&edge.range), "validateSession(token)");
    }
    let limited = engine
        .references(ReferencesRequest {
            limit: 1,
            ..references(SESSION)
        })
        .unwrap();
    assert_eq!(limited.results, result.results[..1]);
    assert_partial(&limited.meta, PartialResultReason::ResultLimit);

    let leaf = engine.references(references(ROUTER)).unwrap();
    assert!(leaf.results.is_empty());
    assert!(!leaf.meta.truncated);
}

#[test]
fn impact_is_ranked_and_explanation_paths_follow_consumer_to_dependency_edges() {
    let engine = MockEngine::default();
    let response = engine.impact(impact(SESSION)).unwrap();
    let repo_graph = engine.graph(graph(ROUTER)).unwrap();
    assert_eq!(
        response
            .results
            .iter()
            .map(|candidate| candidate.symbol.id.0.as_str())
            .collect::<Vec<_>>(),
        [MIDDLEWARE, AUDIT, ROUTER]
    );
    assert!(
        response
            .results
            .windows(2)
            .all(|pair| pair[0].score > pair[1].score)
    );

    for (candidate, hops) in response.results.iter().zip([1, 1, 2]) {
        assert_eq!(candidate.score_meaning, ScoreMeaning::RankingHeuristic);
        assert!(candidate.score.is_finite());
        assert_eq!(candidate.evidence_unavailable, None);
        assert_eq!(candidate.paths.len(), 1);
        let path = &candidate.paths[0];
        assert_eq!(path.len(), hops);
        assert_eq!(path[0].consumer, candidate.symbol.id);
        assert_eq!(
            path.last().unwrap().dependency,
            Some(SymbolId(SESSION.into()))
        );
        assert!(
            path.windows(2)
                .all(|pair| pair[0].dependency.as_ref() == Some(&pair[1].consumer))
        );
        for edge in path {
            assert!(repo_graph.edges.contains(edge));
            assert!(source_snippet(&edge.range).ends_with(')'));
        }
    }

    let other = engine.impact(impact(MIDDLEWARE)).unwrap();
    assert_eq!(other.results.len(), 1);
    assert_eq!(other.results[0].symbol.id.0, ROUTER);
    assert!(engine.impact(impact(ROUTER)).unwrap().results.is_empty());
}

#[test]
fn impact_bounds_keep_rank_and_report_what_was_cut_off() {
    let engine = MockEngine::default();
    let full = engine.impact(impact(SESSION)).unwrap();
    assert_eq!(full.reached_nodes, full.results.len() as u64);
    assert_eq!(full.processed_nodes, full.reached_nodes);

    let cases = [
        (
            ImpactRequest {
                limit: 1,
                ..impact(SESSION)
            },
            PartialResultReason::ResultLimit,
            1,
            full.reached_nodes,
        ),
        (
            ImpactRequest {
                max_hops: 1,
                ..impact(SESSION)
            },
            PartialResultReason::HopLimit,
            2,
            2,
        ),
        (
            ImpactRequest {
                node_budget: 1,
                ..impact(SESSION)
            },
            PartialResultReason::NodeBudget,
            1,
            1,
        ),
        (
            ImpactRequest {
                deadline_ms: Some(0),
                ..impact(SESSION)
            },
            PartialResultReason::Deadline,
            0,
            0,
        ),
    ];
    for (request, reason, len, reached) in cases {
        let response = engine.impact(request).unwrap();
        assert_partial(&response.meta, reason);
        assert_eq!(response.results, full.results[..len]);
        assert_eq!(response.reached_nodes, reached);
        assert!(response.processed_nodes <= response.reached_nodes);
    }

    let scripted = MockEngine::partial().impact(impact(SESSION)).unwrap();
    assert_partial(&scripted.meta, PartialResultReason::Deadline);
    assert_eq!(scripted.results, full.results[..1]);
    assert!(scripted.processed_nodes < scripted.reached_nodes);
}

#[test]
fn graph_keeps_cycle_direction_and_excludes_disconnected_or_bounded_nodes() {
    let engine = MockEngine::default();
    let all = engine.graph(graph(ROUTER)).unwrap();
    assert!(!all.meta.truncated);
    for node in &all.nodes {
        assert!(
            source_snippet(&node.range).starts_with(&format!("export function {}(", node.name))
        );
    }
    assert!(all.edges.iter().all(|edge| {
        all.nodes.iter().any(|node| node.id == edge.consumer)
            && all
                .nodes
                .iter()
                .any(|node| Some(&node.id) == edge.dependency.as_ref())
    }));
    let links: Vec<_> = all
        .edges
        .iter()
        .map(|edge| {
            (
                edge.consumer.0.as_str(),
                edge.dependency.as_ref().unwrap().0.as_str(),
            )
        })
        .collect();
    assert!(links.contains(&(NORMALIZE, PREFIX)) && links.contains(&(PREFIX, NORMALIZE)));
    assert!(links.contains(&(MIDDLEWARE, SESSION)) && !links.contains(&(SESSION, MIDDLEWARE)));

    let nearby = engine
        .graph(GraphRequest {
            max_hops: 1,
            ..graph(SESSION)
        })
        .unwrap();
    assert_eq!(symbol_ids(&nearby.nodes), [AUDIT, MIDDLEWARE, SESSION]);
    assert_eq!(
        nearby.edges,
        engine.references(references(SESSION)).unwrap().results
    );
    assert_partial(&nearby.meta, PartialResultReason::HopLimit);

    for (request, reason) in [
        (
            GraphRequest {
                limit: 1,
                ..graph(SESSION)
            },
            PartialResultReason::ResultLimit,
        ),
        (
            GraphRequest {
                node_budget: 1,
                ..graph(SESSION)
            },
            PartialResultReason::NodeBudget,
        ),
    ] {
        let response = engine.graph(request).unwrap();
        assert_eq!(symbol_ids(&response.nodes), [SESSION]);
        assert!(response.edges.is_empty());
        assert_partial(&response.meta, reason);
    }

    let zero = engine
        .graph(GraphRequest {
            limit: 0,
            ..graph(SESSION)
        })
        .unwrap();
    assert!(zero.nodes.is_empty() && zero.edges.is_empty());
    assert_partial(&zero.meta, PartialResultReason::ResultLimit);
}

#[test]
fn ambiguous_names_require_an_exact_id_and_unresolved_edges_keep_their_evidence() {
    let engine = MockEngine::load(Scenario::EdgeCases).unwrap();
    let matches = engine.search(search("validateSession")).unwrap().results;
    let [legacy, current] = matches.as_slice() else {
        panic!("expected two exact matches: {matches:?}");
    };
    assert_ne!(legacy.id, current.id);
    assert_eq!(current.id.0, SESSION);
    let path = &legacy.range.repo_relative_file.0;
    assert!(path.len() > 260);
    assert!(path.contains("旧セッション"));
    assert!(source_snippet(&legacy.range).starts_with("export function validateSession("));
    assert_eq!(
        engine.search(search("旧セッション")).unwrap().results,
        std::slice::from_ref(legacy)
    );

    assert_eq!(
        engine.references(references("validateSession")),
        Err(EngineError::AmbiguousTarget)
    );
    assert_eq!(
        engine.impact(impact("validateSession")),
        Err(EngineError::AmbiguousTarget)
    );
    assert_eq!(
        engine.graph(graph("validateSession")),
        Err(EngineError::AmbiguousTarget)
    );
    assert_eq!(
        engine.graph(graph(&legacy.id.0)).unwrap().nodes,
        std::slice::from_ref(legacy)
    );

    let graph = engine.graph(graph("runValidation")).unwrap();
    assert_eq!(symbol_ids(&graph.nodes), ["src/adapters.js::runValidation"]);
    let [ambiguous, unresolved] = graph.edges.as_slice() else {
        panic!("expected ambiguous and unresolved evidence");
    };
    assert_eq!(ambiguous.resolution, ResolutionState::Ambiguous);
    assert_eq!(unresolved.resolution, ResolutionState::Unresolved);
    assert_eq!(ambiguous.dependency, None);
    assert_eq!(unresolved.dependency, None);
    assert_eq!(source_snippet(&ambiguous.range), "validate(token)");
    assert_eq!(source_snippet(&unresolved.range), "adapters.current(token)");

    let result = engine.impact(impact(SESSION)).unwrap();
    assert_eq!(result.meta.unresolved_count, graph.edges.len() as u64);
    assert_eq!(result.meta.snapshot_epoch, graph.meta.snapshot_epoch);
    assert!(!result.meta.truncated);
    assert_eq!(
        result.results,
        MockEngine::default()
            .impact(impact(SESSION))
            .unwrap()
            .results
    );
}

#[test]
fn stale_snapshot_allows_the_retained_epoch_and_rejects_mismatches() {
    let engine = MockEngine::stale();
    let health = engine.health().unwrap();
    let epoch = Some(health.meta.snapshot_epoch.clone());
    for meta in [
        engine
            .search(SearchRequest {
                expected_snapshot: epoch.clone(),
                ..search("session")
            })
            .unwrap()
            .meta,
        engine
            .references(ReferencesRequest {
                expected_snapshot: epoch.clone(),
                ..references(SESSION)
            })
            .unwrap()
            .meta,
        engine
            .impact(ImpactRequest {
                expected_snapshot: epoch.clone(),
                ..impact(SESSION)
            })
            .unwrap()
            .meta,
        engine
            .graph(GraphRequest {
                expected_snapshot: epoch,
                ..graph(ROUTER)
            })
            .unwrap()
            .meta,
    ] {
        assert_eq!(meta, health.meta);
    }

    for engine in [
        MockEngine::default(),
        MockEngine::stale(),
        MockEngine::empty(),
        MockEngine::partial(),
        MockEngine::load(Scenario::EdgeCases).unwrap(),
    ] {
        let wrong = Some(SnapshotId(
            engine.health().unwrap().meta.snapshot_epoch.0 + 1,
        ));
        assert_eq!(
            engine.search(SearchRequest {
                expected_snapshot: wrong.clone(),
                ..search("session")
            }),
            Err(EngineError::StaleSnapshot)
        );
        assert_eq!(
            engine.references(ReferencesRequest {
                expected_snapshot: wrong.clone(),
                ..references(SESSION)
            }),
            Err(EngineError::StaleSnapshot)
        );
        assert_eq!(
            engine.impact(ImpactRequest {
                expected_snapshot: wrong.clone(),
                ..impact(SESSION)
            }),
            Err(EngineError::StaleSnapshot)
        );
        assert_eq!(
            engine.graph(GraphRequest {
                expected_snapshot: wrong,
                ..graph(ROUTER)
            }),
            Err(EngineError::StaleSnapshot)
        );
    }
}

#[test]
fn invalid_unknown_and_unavailable_requests_use_contract_errors() {
    let engine = MockEngine::default();
    assert_eq!(
        engine.search(search("  ")),
        Err(EngineError::InvalidRequest)
    );
    assert_eq!(
        engine.references(references("")),
        Err(EngineError::InvalidRequest)
    );
    assert_eq!(engine.impact(impact("")), Err(EngineError::InvalidRequest));
    assert_eq!(engine.graph(graph("")), Err(EngineError::InvalidRequest));
    assert_eq!(
        engine.references(references("absent")),
        Err(EngineError::UnknownTarget)
    );
    assert_eq!(
        engine.impact(impact("absent")),
        Err(EngineError::UnknownTarget)
    );
    assert_eq!(
        engine.graph(graph("absent")),
        Err(EngineError::UnknownTarget)
    );

    let engine = MockEngine::load(Scenario::Unavailable).unwrap();
    assert_eq!(engine.health(), Err(EngineError::Unavailable));
    assert_eq!(
        engine.search(search("session")),
        Err(EngineError::Unavailable)
    );
    assert_eq!(
        engine.references(references(SESSION)),
        Err(EngineError::Unavailable)
    );
    assert_eq!(
        engine.impact(impact(SESSION)),
        Err(EngineError::Unavailable)
    );
    assert_eq!(engine.graph(graph(SESSION)), Err(EngineError::Unavailable));
}

#[test]
fn shared_trait_object_returns_identical_results_across_threads() {
    fn responses(
        engine: &dyn EngineApi,
    ) -> (
        IndexHealth,
        SearchResponse,
        ReferencesResponse,
        ImpactResponse,
        GraphResponse,
    ) {
        (
            engine.health().unwrap(),
            engine.search(search("session")).unwrap(),
            engine.references(references(SESSION)).unwrap(),
            engine.impact(impact(SESSION)).unwrap(),
            engine.graph(graph(ROUTER)).unwrap(),
        )
    }

    let engine: Arc<dyn EngineApi> = Arc::new(MockEngine::default());
    let expected = responses(engine.as_ref());
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let engine = Arc::clone(&engine);
            thread::spawn(move || responses(engine.as_ref()))
        })
        .collect();
    for worker in threads {
        assert_eq!(worker.join().unwrap(), expected);
    }
}
