use faultline_contract::*;
pub struct FaultlineEngine;
impl EngineApi for FaultlineEngine {
    fn health(&self) -> Result<IndexHealth, EngineError> {
        Err(EngineError::Unavailable)
    }
    fn search(&self, _req: SearchRequest) -> Result<SearchResponse, EngineError> {
        Err(EngineError::Unavailable)
    }
    fn references(&self, _req: ReferencesRequest) -> Result<ReferencesResponse, EngineError> {
        Err(EngineError::Unavailable)
    }
    fn impact(&self, _req: ImpactRequest) -> Result<ImpactResponse, EngineError> {
        Err(EngineError::Unavailable)
    }
    fn graph(&self, _req: GraphRequest) -> Result<GraphResponse, EngineError> {
        Err(EngineError::Unavailable)
    }
}
