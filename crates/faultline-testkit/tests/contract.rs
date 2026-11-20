use faultline_contract::*;
use faultline_testkit::{MockEngine, Scenario};
use serde_json::Value;

macro_rules! fixture {
    ($path:literal, $dto:ty) => {{
        let json = include_str!(concat!("../../../fixtures/", $path));
        let original: Value = serde_json::from_str(json).expect($path);
        let dto: $dto = serde_json::from_str(json).expect($path);
        let serialized = serde_json::to_string(&dto).expect($path);
        assert_eq!(
            serde_json::from_str::<Value>(&serialized).expect($path),
            original,
            "{} differs from the contract serialization",
            $path
        );
        let round_trip: $dto = serde_json::from_str(&serialized).expect($path);
        assert_eq!(round_trip, dto, "{}", $path);
        dto
    }};
}

fn check_meta(meta: &ResponseMeta, health: &IndexHealth) {
    assert_eq!(meta.schema_version, SCHEMA_VERSION);
    assert_eq!(meta.snapshot_epoch, health.meta.snapshot_epoch);
    assert_eq!(meta.freshness, health.meta.freshness);
    assert_eq!(meta.unresolved_count, health.meta.unresolved_count);
    assert_eq!(meta.truncated, meta.partial_reason.is_some());
}

#[test]
fn bootstrap_snapshot_round_trips_through_public_api() {
    let mock = MockEngine::empty();
    let engine: &dyn EngineApi = &mock;
    let target = SymbolId("x".into());
    let health = fixture!("bootstrap/health.json", IndexHealth);
    let epoch = Some(health.meta.snapshot_epoch.clone());
    assert_eq!(engine.health().unwrap(), health);

    let search = fixture!("bootstrap/search.json", SearchResponse);
    check_meta(&search.meta, &health);
    assert_eq!(
        engine
            .search(SearchRequest {
                query: "x".into(),
                limit: 10,
                expected_snapshot: epoch.clone(),
            })
            .unwrap(),
        search
    );

    let refs = fixture!("bootstrap/references.json", ReferencesResponse);
    check_meta(&refs.meta, &health);
    assert_eq!(
        engine
            .references(ReferencesRequest {
                target: target.clone(),
                limit: 10,
                expected_snapshot: epoch.clone(),
            })
            .unwrap(),
        refs
    );

    let impact = fixture!("bootstrap/impact.json", ImpactResponse);
    check_meta(&impact.meta, &health);
    assert_eq!(
        engine
            .impact(ImpactRequest {
                target: target.clone(),
                max_hops: 2,
                node_budget: 10,
                deadline_ms: None,
                limit: 10,
                expected_snapshot: epoch.clone(),
            })
            .unwrap(),
        impact
    );

    let graph = fixture!("bootstrap/graph.json", GraphResponse);
    check_meta(&graph.meta, &health);
    assert_eq!(
        engine
            .graph(GraphRequest {
                target,
                max_hops: 2,
                node_budget: 10,
                limit: 10,
                expected_snapshot: epoch,
            })
            .unwrap(),
        graph
    );
}

#[test]
fn healthy_snapshot_matches_goldens_for_all_five_methods() {
    let mock = MockEngine::default();
    let engine: &dyn EngineApi = &mock;
    let health = fixture!("demo/health.json", IndexHealth);
    let epoch = Some(health.meta.snapshot_epoch.clone());
    assert_eq!(engine.health().unwrap(), health);

    for (query, golden) in [
        ("session", fixture!("demo/search.json", SearchResponse)),
        (
            "missing",
            fixture!("demo/empty-search.json", SearchResponse),
        ),
    ] {
        check_meta(&golden.meta, &health);
        assert_eq!(
            engine
                .search(SearchRequest {
                    query: query.into(),
                    limit: 10,
                    expected_snapshot: epoch.clone(),
                })
                .unwrap(),
            golden
        );
    }

    let refs = fixture!("demo/references.json", ReferencesResponse);
    check_meta(&refs.meta, &health);
    assert_eq!(
        engine
            .references(ReferencesRequest {
                target: SymbolId("src/session.ts::validateSession".into()),
                limit: 10,
                expected_snapshot: epoch.clone(),
            })
            .unwrap(),
        refs
    );

    for (target, golden) in [
        (
            "src/session.ts::validateSession",
            fixture!("demo/impact/validate-session.json", ImpactResponse),
        ),
        (
            "src/middleware.ts::authorizeRequest",
            fixture!("demo/impact/authorize-request.json", ImpactResponse),
        ),
        (
            "src/cycle/normalize.js::normalizeToken",
            fixture!("demo/impact/normalize-token.json", ImpactResponse),
        ),
        (
            "src/cycle/prefix.js::stripPrefix",
            fixture!("demo/impact/strip-prefix.json", ImpactResponse),
        ),
    ] {
        check_meta(&golden.meta, &health);
        assert_eq!(
            engine
                .impact(ImpactRequest {
                    target: SymbolId(target.into()),
                    max_hops: 3,
                    node_budget: 10,
                    deadline_ms: None,
                    limit: 10,
                    expected_snapshot: epoch.clone(),
                })
                .unwrap(),
            golden
        );
    }

    let graph = fixture!("demo/graph.json", GraphResponse);
    check_meta(&graph.meta, &health);
    assert_eq!(
        engine
            .graph(GraphRequest {
                target: SymbolId("src/router.ts::routeRequest".into()),
                max_hops: 3,
                node_budget: 10,
                limit: 10,
                expected_snapshot: epoch,
            })
            .unwrap(),
        graph
    );
}

#[test]
fn edge_case_goldens_preserve_metadata_and_partial_results() {
    let stale = fixture!("demo/edge-cases/stale-health.json", IndexHealth);
    check_meta(&stale.meta, &stale);
    assert_eq!(MockEngine::stale().health().unwrap(), stale);

    let partial = fixture!("demo/edge-cases/partial-impact.json", ImpactResponse);
    let engine = MockEngine::partial();
    check_meta(&partial.meta, &engine.health().unwrap());
    assert_eq!(
        engine
            .impact(ImpactRequest {
                target: SymbolId("validateSession".into()),
                max_hops: 3,
                node_budget: 10,
                deadline_ms: None,
                limit: 10,
                expected_snapshot: Some(partial.meta.snapshot_epoch.clone()),
            })
            .unwrap(),
        partial
    );

    let health = fixture!("demo/edge-cases/health.json", IndexHealth);
    let graph = fixture!("demo/edge-cases/graph.json", GraphResponse);
    check_meta(&graph.meta, &health);
    let engine = MockEngine::load(Scenario::EdgeCases).unwrap();
    assert_eq!(engine.health().unwrap(), health);
    // The two supplemental nodes have only ambiguous/unresolved edges between
    // them and the rest of the snapshot, so neither neighbors the other.
    for node in graph.nodes {
        let response = engine
            .graph(GraphRequest {
                target: node.id.clone(),
                max_hops: 3,
                node_budget: 10,
                limit: 10,
                expected_snapshot: Some(health.meta.snapshot_epoch.clone()),
            })
            .unwrap();
        check_meta(&response.meta, &health);
        assert_eq!(response.nodes, std::slice::from_ref(&node));
        assert_eq!(
            response.edges,
            graph
                .edges
                .iter()
                .filter(|edge| edge.consumer == node.id)
                .cloned()
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn error_goldens_use_the_frozen_wire_values() {
    let errors = [
        fixture!("demo/errors/unavailable.json", EngineError),
        fixture!("demo/errors/unknown-target.json", EngineError),
        fixture!("demo/errors/invalid-request.json", EngineError),
        fixture!("demo/errors/ambiguous-target.json", EngineError),
        fixture!("demo/errors/stale-snapshot.json", EngineError),
    ];
    assert_eq!(
        MockEngine::load(Scenario::Unavailable).unwrap().health(),
        Err(errors[0].clone())
    );
    assert_eq!(
        errors,
        [
            EngineError::Unavailable,
            EngineError::UnknownTarget,
            EngineError::InvalidRequest,
            EngineError::AmbiguousTarget,
            EngineError::StaleSnapshot,
        ]
    );
}
