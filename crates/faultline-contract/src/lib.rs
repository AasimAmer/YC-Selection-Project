//! Owned wire types for FAULTLINE. Lines start at one; byte offsets are zero-based and end-exclusive.
use serde::{Deserialize, Serialize};
pub const SCHEMA_VERSION: u32 = 1;
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct SnapshotId(pub u64);
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct SymbolId(pub String);
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct FileId(pub String);
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Current,
    Stale,
    Indexing,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Method,
    Variable,
    Interface,
    Type,
    Module,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Calls,
    Imports,
    Exports,
    Reexports,
    TypeRef,
    Extends,
    Implements,
    TestReference,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolutionState {
    Resolved,
    Ambiguous,
    External,
    Unresolved,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialResultReason {
    Deadline,
    NodeBudget,
    HopLimit,
    ResultLimit,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Concern {
    High,
    Medium,
    Low,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreMeaning {
    RankingHeuristic,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineError {
    Unavailable,
    StaleSnapshot,
    AmbiguousTarget,
    UnknownTarget,
    InvalidRequest,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct SourceLocation {
    pub line: u32,
    pub byte: u64,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct SourceRange {
    pub repo_relative_file: FileId,
    pub start: SourceLocation,
    pub end: SourceLocation,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub range: SourceRange,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct EvidenceEdge {
    pub consumer: SymbolId,
    pub dependency: Option<SymbolId>,
    pub kind: EdgeKind,
    pub resolution: ResolutionState,
    pub range: SourceRange,
}
pub type GraphEdge = EvidenceEdge;
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct ImpactCandidate {
    pub symbol: GraphNode,
    pub score: f64,
    pub score_meaning: ScoreMeaning,
    pub concern: Concern,
    pub paths: Vec<Vec<EvidenceEdge>>,
    pub evidence_unavailable: Option<String>,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct ResponseMeta {
    pub schema_version: u32,
    pub snapshot_epoch: SnapshotId,
    pub freshness: Freshness,
    pub truncated: bool,
    pub partial_reason: Option<PartialResultReason>,
    pub unresolved_count: u64,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct IndexHealth {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub files: u64,
    pub symbols: u64,
    pub resolved_edges: u64,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub limit: u32,
    pub expected_snapshot: Option<SnapshotId>,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct SearchResponse {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub results: Vec<GraphNode>,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct ReferencesRequest {
    pub target: SymbolId,
    pub limit: u32,
    pub expected_snapshot: Option<SnapshotId>,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct ReferencesResponse {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub results: Vec<EvidenceEdge>,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct ImpactRequest {
    pub target: SymbolId,
    pub max_hops: u32,
    pub node_budget: u64,
    pub deadline_ms: Option<u64>,
    pub limit: u32,
    pub expected_snapshot: Option<SnapshotId>,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct ImpactResponse {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub results: Vec<ImpactCandidate>,
    pub processed_nodes: u64,
    pub reached_nodes: u64,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct GraphRequest {
    pub target: SymbolId,
    pub max_hops: u32,
    pub node_budget: u64,
    pub limit: u32,
    pub expected_snapshot: Option<SnapshotId>,
}
#[derive(Clone, Debug, PartialEq, schemars::JsonSchema, Serialize, Deserialize)]
pub struct GraphResponse {
    #[serde(flatten)]
    pub meta: ResponseMeta,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}
pub trait EngineApi: Send + Sync {
    fn health(&self) -> Result<IndexHealth, EngineError>;
    fn search(&self, req: SearchRequest) -> Result<SearchResponse, EngineError>;
    fn references(&self, req: ReferencesRequest) -> Result<ReferencesResponse, EngineError>;
    fn impact(&self, req: ImpactRequest) -> Result<ImpactResponse, EngineError>;
    fn graph(&self, req: GraphRequest) -> Result<GraphResponse, EngineError>;
}
