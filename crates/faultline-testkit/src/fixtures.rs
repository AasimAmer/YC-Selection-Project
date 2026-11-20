use std::{error::Error, fmt};

use faultline_contract::{
    EngineError, Freshness, GraphResponse, ImpactResponse, IndexHealth, ResponseMeta,
    SCHEMA_VERSION,
};

/// Selects one of the embedded demo snapshots or a fixture-backed error.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Scenario {
    #[default]
    Healthy,
    Empty,
    Stale,
    Partial,
    EdgeCases,
    Unavailable,
}

/// Failure to decode or validate an embedded fixture.
#[derive(Debug)]
pub struct FixtureError {
    /// Repository-relative path of the fixture that failed to load.
    pub path: &'static str,
    message: String,
}

impl fmt::Display for FixtureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

impl Error for FixtureError {}

macro_rules! read_fixture {
    ($path:literal, $dto:ty) => {
        serde_json::from_str::<$dto>(include_str!(concat!("../../../", $path))).map_err(|error| {
            FixtureError {
                path: $path,
                message: error.to_string(),
            }
        })
    };
}

pub(crate) fn unavailable_error() -> Result<EngineError, FixtureError> {
    read_fixture!("fixtures/demo/errors/unavailable.json", EngineError)
}

#[derive(Debug)]
pub(crate) struct Fixture {
    pub health: IndexHealth,
    pub graph: GraphResponse,
    pub impacts: Vec<(&'static str, ImpactResponse)>,
}

impl Fixture {
    pub fn load(scenario: Scenario) -> Result<Self, FixtureError> {
        if scenario == Scenario::Empty {
            let health = read_fixture!("fixtures/bootstrap/health.json", IndexHealth)?;
            let graph = read_fixture!("fixtures/bootstrap/graph.json", GraphResponse)?;
            check_meta("fixtures/bootstrap/health.json", &health.meta, &health.meta)?;
            check_meta("fixtures/bootstrap/graph.json", &graph.meta, &health.meta)?;
            return Ok(Self {
                health,
                graph,
                impacts: Vec::new(),
            });
        }

        let mut health = read_fixture!("fixtures/demo/health.json", IndexHealth)?;
        let mut graph = read_fixture!("fixtures/demo/graph.json", GraphResponse)?;
        check_meta("fixtures/demo/health.json", &health.meta, &health.meta)?;
        check_meta("fixtures/demo/graph.json", &graph.meta, &health.meta)?;

        macro_rules! load_impact {
            ($path:literal) => {{
                let impact = read_fixture!($path, ImpactResponse)?;
                check_meta($path, &impact.meta, &health.meta)?;
                impact
            }};
        }
        let session_impact = if scenario == Scenario::Partial {
            load_impact!("fixtures/demo/edge-cases/partial-impact.json")
        } else {
            load_impact!("fixtures/demo/impact/validate-session.json")
        };
        let impacts = vec![
            ("src/session.ts::validateSession", session_impact),
            (
                "src/middleware.ts::authorizeRequest",
                load_impact!("fixtures/demo/impact/authorize-request.json"),
            ),
            (
                "src/cycle/normalize.js::normalizeToken",
                load_impact!("fixtures/demo/impact/normalize-token.json"),
            ),
            (
                "src/cycle/prefix.js::stripPrefix",
                load_impact!("fixtures/demo/impact/strip-prefix.json"),
            ),
        ];

        if scenario == Scenario::Stale {
            let stale = read_fixture!("fixtures/demo/edge-cases/stale-health.json", IndexHealth)?;
            let expected = ResponseMeta {
                freshness: Freshness::Stale,
                ..health.meta.clone()
            };
            check_meta(
                "fixtures/demo/edge-cases/stale-health.json",
                &stale.meta,
                &expected,
            )?;
            health = stale;
        } else if scenario == Scenario::EdgeCases {
            health = read_fixture!("fixtures/demo/edge-cases/health.json", IndexHealth)?;
            let extra = read_fixture!("fixtures/demo/edge-cases/graph.json", GraphResponse)?;
            check_meta(
                "fixtures/demo/edge-cases/health.json",
                &health.meta,
                &health.meta,
            )?;
            check_meta(
                "fixtures/demo/edge-cases/graph.json",
                &extra.meta,
                &health.meta,
            )?;
            graph.nodes.extend(extra.nodes);
            graph.edges.extend(extra.edges);
        }

        Ok(Self {
            health,
            graph,
            impacts,
        })
    }
}

fn check_meta(
    path: &'static str,
    actual: &ResponseMeta,
    snapshot: &ResponseMeta,
) -> Result<(), FixtureError> {
    let message = if actual.schema_version != SCHEMA_VERSION {
        format!(
            "unsupported schema version {} (expected {SCHEMA_VERSION})",
            actual.schema_version
        )
    } else if actual.snapshot_epoch != snapshot.snapshot_epoch
        || actual.freshness != snapshot.freshness
        || actual.unresolved_count != snapshot.unresolved_count
    {
        "snapshot metadata disagrees with health".into()
    } else if actual.truncated != actual.partial_reason.is_some() {
        "truncation flag disagrees with partial reason".into()
    } else {
        return Ok(());
    };
    Err(FixtureError { path, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_schema_version_in_a_deserializable_response() {
        let wrong = read_fixture!("fixtures/demo/errors/schema-mismatch.json", IndexHealth)
            .expect("the fixture should deserialize before version validation");
        let error = check_meta(
            "fixtures/demo/errors/schema-mismatch.json",
            &wrong.meta,
            &wrong.meta,
        )
        .unwrap_err();
        assert_ne!(wrong.meta.schema_version, SCHEMA_VERSION);
        assert!(error.to_string().contains("unsupported schema version"));
        assert_eq!(error.path, "fixtures/demo/errors/schema-mismatch.json");
    }

    #[test]
    fn rejects_mixed_snapshots_and_inconsistent_partial_metadata() {
        let health = read_fixture!("fixtures/demo/health.json", IndexHealth).unwrap();
        let mut meta = health.meta.clone();
        meta.snapshot_epoch.0 += 1;
        assert!(check_meta("mixed.json", &meta, &health.meta).is_err());
        meta.snapshot_epoch = health.meta.snapshot_epoch.clone();
        meta.truncated = true;
        assert!(check_meta("partial.json", &meta, &health.meta).is_err());
    }
}
