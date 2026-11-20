//! Fixture-backed FAULTLINE API for testing consumers without an index or parser.
mod fixtures;

use std::sync::Arc;

use faultline_contract::{
    EngineApi, EngineError, EvidenceEdge, GraphRequest, GraphResponse, ImpactCandidate,
    ImpactRequest, ImpactResponse, IndexHealth, PartialResultReason, ReferencesRequest,
    ReferencesResponse, ResponseMeta, SearchRequest, SearchResponse, SnapshotId, SymbolId,
};
use fixtures::Fixture;

pub use fixtures::{FixtureError, Scenario};

/// An immutable demo snapshot implementing the public engine API.
///
/// Use [`Self::default`] for a healthy snapshot or [`Self::load`] to select a
/// scenario and handle fixture errors. Clones share the parsed fixture.
#[derive(Clone, Debug)]
pub struct MockEngine {
    state: Result<Arc<Fixture>, EngineError>,
}

impl Default for MockEngine {
    fn default() -> Self {
        Self::builtin(Scenario::Healthy)
    }
}

impl MockEngine {
    /// Loads and validates an embedded fixture. Queries require no filesystem access.
    pub fn load(scenario: Scenario) -> Result<Self, FixtureError> {
        if scenario == Scenario::Unavailable {
            return Ok(Self::error(fixtures::unavailable_error()?));
        }
        Ok(Self {
            state: Ok(Arc::new(Fixture::load(scenario)?)),
        })
    }

    /// Returns a current, empty snapshot.
    pub fn empty() -> Self {
        Self::builtin(Scenario::Empty)
    }

    /// Retains the healthy snapshot data and reports stale freshness.
    pub fn stale() -> Self {
        Self::builtin(Scenario::Stale)
    }

    /// Returns a scripted deadline cutoff for impact on `validateSession`.
    pub fn partial() -> Self {
        Self::builtin(Scenario::Partial)
    }

    /// Returns the supplied error from every API call.
    pub fn error(error: EngineError) -> Self {
        Self { state: Err(error) }
    }

    fn builtin(scenario: Scenario) -> Self {
        Self::load(scenario).unwrap_or_else(|error| panic!("invalid embedded fixture: {error}"))
    }

    fn fixture(&self, expected: Option<&SnapshotId>) -> Result<&Fixture, EngineError> {
        let fixture = self.state.as_deref().map_err(Clone::clone)?;
        if expected.is_some_and(|id| *id != fixture.health.meta.snapshot_epoch) {
            return Err(EngineError::StaleSnapshot);
        }
        Ok(fixture)
    }
}

impl EngineApi for MockEngine {
    fn health(&self) -> Result<IndexHealth, EngineError> {
        Ok(self.fixture(None)?.health.clone())
    }

    fn search(&self, req: SearchRequest) -> Result<SearchResponse, EngineError> {
        let fixture = self.fixture(req.expected_snapshot.as_ref())?;
        let query = req.query.trim().to_lowercase();
        if query.is_empty() {
            return Err(EngineError::InvalidRequest);
        }

        let mut matches: Vec<_> = fixture
            .graph
            .nodes
            .iter()
            .filter_map(|node| {
                let name = node.name.to_lowercase();
                let path = &node.range.repo_relative_file.0;
                let rank = if name == query {
                    0
                } else if name.starts_with(&query) {
                    1
                } else if name.contains(&query) {
                    2
                } else if path.to_lowercase().contains(&query) {
                    3
                } else {
                    return None;
                };
                Some((rank, path, &node.id.0, node))
            })
            .collect();
        matches.sort_by_key(|(rank, path, id, _)| (*rank, *path, *id));
        let mut meta = fixture.health.meta.clone();
        limit(&mut matches, req.limit as usize, &mut meta);
        Ok(SearchResponse {
            meta,
            results: matches
                .into_iter()
                .map(|(_, _, _, node)| node.clone())
                .collect(),
        })
    }

    fn references(&self, req: ReferencesRequest) -> Result<ReferencesResponse, EngineError> {
        let fixture = self.fixture(req.expected_snapshot.as_ref())?;
        let target = resolve_target(fixture, &req.target)?;
        let mut response = ReferencesResponse {
            meta: fixture.health.meta.clone(),
            results: target.map_or_else(Vec::new, |target| {
                fixture
                    .graph
                    .edges
                    .iter()
                    .filter(|edge| edge.dependency.as_ref() == Some(target))
                    .cloned()
                    .collect()
            }),
        };
        limit(
            &mut response.results,
            req.limit as usize,
            &mut response.meta,
        );
        Ok(response)
    }

    fn impact(&self, req: ImpactRequest) -> Result<ImpactResponse, EngineError> {
        let fixture = self.fixture(req.expected_snapshot.as_ref())?;
        let target = resolve_target(fixture, &req.target)?;
        let mut response = ImpactResponse {
            meta: fixture.health.meta.clone(),
            results: Vec::new(),
            processed_nodes: 0,
            reached_nodes: 0,
        };
        let Some(target) = target else {
            return Ok(response);
        };
        if req.deadline_ms == Some(0) {
            mark_partial(&mut response.meta, PartialResultReason::Deadline);
            return Ok(response);
        }
        if let Some((_, scripted)) = fixture.impacts.iter().find(|(id, _)| *id == target.0) {
            response.results.clone_from(&scripted.results);
            response.processed_nodes = scripted.processed_nodes;
            response.reached_nodes = scripted.reached_nodes;
            bound_impact(&mut response, req.max_hops, req.node_budget);
            if let Some(reason) = &scripted.meta.partial_reason {
                mark_partial(&mut response.meta, reason.clone());
            }
        }
        limit(
            &mut response.results,
            req.limit as usize,
            &mut response.meta,
        );
        Ok(response)
    }

    fn graph(&self, req: GraphRequest) -> Result<GraphResponse, EngineError> {
        let fixture = self.fixture(req.expected_snapshot.as_ref())?;
        let target = resolve_target(fixture, &req.target)?;
        let mut response = GraphResponse {
            meta: fixture.health.meta.clone(),
            nodes: Vec::new(),
            edges: Vec::new(),
        };
        let Some(target) = target else {
            return Ok(response);
        };
        let budget = usize::try_from(req.node_budget).unwrap_or(usize::MAX);
        let limit = req.limit as usize;
        let cap = budget.min(limit);
        let (nodes, reason) = graph_neighborhood(&fixture.graph.edges, target, req.max_hops, cap);
        if let Some(reason) = reason {
            let reason = if reason == PartialResultReason::NodeBudget && limit < budget {
                PartialResultReason::ResultLimit
            } else {
                reason
            };
            mark_partial(&mut response.meta, reason);
        }
        response.nodes = fixture
            .graph
            .nodes
            .iter()
            .filter(|node| nodes.contains(&&node.id))
            .cloned()
            .collect();
        response.edges = fixture
            .graph
            .edges
            .iter()
            .filter(|edge| {
                nodes.contains(&&edge.consumer)
                    && edge
                        .dependency
                        .as_ref()
                        .is_none_or(|id| nodes.contains(&id))
            })
            .cloned()
            .collect();
        Ok(response)
    }
}

fn resolve_target<'a>(
    fixture: &'a Fixture,
    target: &SymbolId,
) -> Result<Option<&'a SymbolId>, EngineError> {
    if target.0.trim().is_empty() {
        return Err(EngineError::InvalidRequest);
    }
    let nodes = &fixture.graph.nodes;
    if nodes.is_empty() {
        return Ok(None);
    }
    if let Some(node) = nodes.iter().find(|node| node.id == *target) {
        return Ok(Some(&node.id));
    }
    let mut names = nodes.iter().filter(|node| node.name == target.0);
    let node = names.next().ok_or(EngineError::UnknownTarget)?;
    if names.next().is_some() {
        return Err(EngineError::AmbiguousTarget);
    }
    Ok(Some(&node.id))
}

fn mark_partial(meta: &mut ResponseMeta, reason: PartialResultReason) {
    meta.truncated = true;
    meta.partial_reason.get_or_insert(reason);
}

fn limit<T>(results: &mut Vec<T>, max_results: usize, meta: &mut ResponseMeta) {
    if results.len() > max_results {
        results.truncate(max_results);
        mark_partial(meta, PartialResultReason::ResultLimit);
    }
}

fn bound_impact(response: &mut ImpactResponse, max_hops: u32, node_budget: u64) {
    let mut hop_limited = false;
    response.results.retain_mut(|candidate| {
        let had_paths = !candidate.paths.is_empty();
        let before = candidate.paths.len();
        candidate
            .paths
            .retain(|path| path.len() <= max_hops as usize);
        hop_limited |= before != candidate.paths.len();
        !had_paths || !candidate.paths.is_empty()
    });

    let node_limited = response.results.len() as u64 > node_budget;
    if node_limited {
        // Select the nearest consumers first, then restore the authored ranking.
        let mut ranked: Vec<_> = response.results.drain(..).enumerate().collect();
        ranked.sort_by_key(|(_, candidate)| shortest_path(candidate));
        ranked.truncate(usize::try_from(node_budget).unwrap_or(usize::MAX));
        ranked.sort_unstable_by_key(|(rank, _)| *rank);
        response
            .results
            .extend(ranked.into_iter().map(|(_, candidate)| candidate));
        mark_partial(&mut response.meta, PartialResultReason::NodeBudget);
    }
    if hop_limited {
        mark_partial(&mut response.meta, PartialResultReason::HopLimit);
    }
    if node_limited || hop_limited {
        response.processed_nodes = response.results.len() as u64;
        response.reached_nodes = response.processed_nodes;
    }
}

fn shortest_path(candidate: &ImpactCandidate) -> usize {
    candidate
        .paths
        .iter()
        .map(Vec::len)
        .min()
        .unwrap_or(usize::MAX)
}

fn graph_neighborhood<'a>(
    edges: &'a [EvidenceEdge],
    target: &'a SymbolId,
    max_hops: u32,
    cap: usize,
) -> (Vec<&'a SymbolId>, Option<PartialResultReason>) {
    if cap == 0 {
        return (Vec::new(), Some(PartialResultReason::NodeBudget));
    }
    let mut selected = vec![target];
    let mut depths = vec![0];
    let mut index = 0;
    let mut hop_limited = false;

    while let Some(&id) = selected.get(index) {
        let depth = depths[index];
        for edge in edges {
            let neighbor = if edge.consumer == *id {
                edge.dependency.as_ref()
            } else if edge.dependency.as_ref() == Some(id) {
                Some(&edge.consumer)
            } else {
                None
            };
            let Some(neighbor) = neighbor.filter(|neighbor| !selected.contains(neighbor)) else {
                continue;
            };
            if depth == max_hops {
                hop_limited = true;
            } else if selected.len() == cap {
                return (selected, Some(PartialResultReason::NodeBudget));
            } else {
                selected.push(neighbor);
                depths.push(depth + 1);
            }
        }
        index += 1;
    }
    (
        selected,
        hop_limited.then_some(PartialResultReason::HopLimit),
    )
}
