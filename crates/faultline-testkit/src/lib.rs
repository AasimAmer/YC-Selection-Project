use faultline_contract::*;
pub struct MockEngine;
impl EngineApi for MockEngine {
    fn health(&self) -> Result<IndexHealth, EngineError> {
        serde_json::from_str(include_str!("../../../fixtures/bootstrap/health.json"))
            .map_err(|_| EngineError::InvalidRequest)
    }
    fn search(&self, req: SearchRequest) -> Result<SearchResponse, EngineError> {
        if req.expected_snapshot.is_some_and(|id| id.0 != 1) {
            return Err(EngineError::StaleSnapshot);
        }
        serde_json::from_str(include_str!("../../../fixtures/bootstrap/search.json"))
            .map_err(|_| EngineError::InvalidRequest)
    }
    fn references(&self, req: ReferencesRequest) -> Result<ReferencesResponse, EngineError> {
        if req.expected_snapshot.is_some_and(|id| id.0 != 1) {
            return Err(EngineError::StaleSnapshot);
        }
        serde_json::from_str(include_str!("../../../fixtures/bootstrap/references.json"))
            .map_err(|_| EngineError::InvalidRequest)
    }
    fn impact(&self, req: ImpactRequest) -> Result<ImpactResponse, EngineError> {
        if req.expected_snapshot.is_some_and(|id| id.0 != 1) {
            return Err(EngineError::StaleSnapshot);
        }
        serde_json::from_str(include_str!("../../../fixtures/bootstrap/impact.json"))
            .map_err(|_| EngineError::InvalidRequest)
    }
    fn graph(&self, req: GraphRequest) -> Result<GraphResponse, EngineError> {
        if req.expected_snapshot.is_some_and(|id| id.0 != 1) {
            return Err(EngineError::StaleSnapshot);
        }
        serde_json::from_str(include_str!("../../../fixtures/bootstrap/graph.json"))
            .map_err(|_| EngineError::InvalidRequest)
    }
}
