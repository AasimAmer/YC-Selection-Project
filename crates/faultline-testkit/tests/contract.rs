use faultline_contract::*;
use faultline_testkit::MockEngine;
use serde_json::{Value, json, to_value};

#[test]
fn empty_snapshot_round_trips_through_public_api() {
    let engine: &dyn EngineApi = &MockEngine;
    let request = json!({"query":"x","target":"x","expected_snapshot":1,"limit":10,"max_hops":2,"node_budget":100});
    macro_rules! check {
        ($method:ident,$value:expr) => {{
            let fixture: Value = serde_json::from_str(include_str!(concat!(
                "../../../fixtures/bootstrap/",
                stringify!($method),
                ".json"
            )))
            .unwrap();
            assert_eq!(fixture["schema_version"], SCHEMA_VERSION);
            assert_eq!(to_value($value.unwrap()).unwrap(), fixture);
        }};
    }
    check!(health, engine.health());
    check!(
        search,
        engine.search(serde_json::from_value(request.clone()).unwrap())
    );
    check!(
        impact,
        engine.impact(serde_json::from_value(request.clone()).unwrap())
    );
    check!(
        references,
        engine.references(serde_json::from_value(request.clone()).unwrap())
    );
    check!(
        graph,
        engine.graph(serde_json::from_value(request).unwrap())
    );
}

#[test]
fn mismatched_epoch_is_rejected() {
    let request = SearchRequest {
        query: "x".into(),
        limit: 1,
        expected_snapshot: Some(SnapshotId(2)),
    };
    assert_eq!(MockEngine.search(request), Err(EngineError::StaleSnapshot));
}
