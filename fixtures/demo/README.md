# FAULTLINE demo fixtures

These snapshots are synthetic test data. Impact scores are authored ranking
heuristics. The fixtures contain no benchmark measurements.

```rust
use faultline_contract::{EngineApi, SearchRequest};
use faultline_testkit::{MockEngine, Scenario};

let engine = MockEngine::default();
let health = engine.health().unwrap();
let hits = engine.search(SearchRequest {
    query: "validateSession".into(),
    limit: 10,
    expected_snapshot: Some(health.meta.snapshot_epoch),
}).unwrap();
assert_eq!(hits.results.len(), 1);

// In a test, load ambiguous names and unresolved edges with:
let edge_cases = MockEngine::load(Scenario::EdgeCases).unwrap();
assert_eq!(edge_cases.health().unwrap().meta.unresolved_count, 2);
```

Use `MockEngine::empty()`, `MockEngine::stale()`, or `MockEngine::partial()` for
the corresponding built-in snapshot. `MockEngine::error(error)` returns the
chosen contract error from all five methods. `MockEngine::load(scenario)` returns
a `FixtureError` with the fixture path if decoding or metadata validation fails.
The convenience constructors panic with the same diagnostic for broken built-in
fixtures.

## Fixture map

| Scenario | Files | Example query |
| --- | --- | --- |
| Healthy index | [health.json](health.json) and [TS/JS source](repo/src/) | Health reports the six-file snapshot. |
| Search | [search.json](search.json), [empty-search.json](empty-search.json) | Search for `session` or `missing`. |
| References | [references.json](references.json) | Find incoming references to `src/session.ts::validateSession` in the audit and middleware files. |
| Impact | [impact/](impact/) | Change `validateSession` for direct consumers and the transitive `routeRequest` candidate. |
| Graph | [graph.json](graph.json) | Focus `routeRequest` with at least three hops and six nodes to see the whole snapshot, including `normalizeToken` ↔ `stripPrefix`. |
| Errors | [errors/](errors/) | Use a blank query or target, a missing target, a duplicate bare name, or an unavailable engine. |
| Edge cases | [edge-cases/](edge-cases/), [empty bootstrap](../bootstrap/) | Load stale results, the scripted impact cutoff, an empty index, or the edge-case overlay with a long Unicode path and ambiguous and unresolved calls. |

The healthy repository root is `repo/`. For the edge-case snapshot, overlay
`edge-cases/repo/` on that root. The supplemental graph JSON contains just the
two added nodes and their evidence. Source ranges use one-based lines and
zero-based half-open UTF-8 byte offsets.

## Query behavior

- Search uses case-insensitive exact, prefix, and substring name matches, then
  path substring matches. Ties use the repository-relative path and symbol ID.
  Search and references apply the result limit after ordering, including a
  zero limit. Blank searches and targets produce `InvalidRequest`.
- Targeted requests accept an exact ID or a unique, case-sensitive bare name.
  Unknown targets in a populated snapshot produce `UnknownTarget`. A nonblank
  target in the empty snapshot yields an empty response. References return
  incoming resolved edges in fixture order. Edges with no dependency ID can
  appear in the graph at the consumer. The metadata's `unresolved_count`
  counts ambiguous and unresolved edges across the snapshot.
- Impact filters authored candidates and explanation paths. Its node budget
  counts candidates, with nearest candidates admitted first and authored
  ranking preserved in the response. Result limits leave the processed and
  reached counts intact. A zero deadline gives an immediate cutoff. Positive
  deadlines use the fixture's result, including the scripted deadline cutoff
  for `validateSession` in the partial scenario.
- The graph selects neighbors in both directions, with the focus counting
  toward the node budget and result limit. It retains the stored node and edge
  order and exposes edge direction as consumer → dependency.
- A different `expected_snapshot` returns `StaleSnapshot`. A matching
  expectation on a stale snapshot returns its retained data with
  `freshness: "stale"`. Snapshot metadata stays consistent across calls.
- The contract exposes one partial reason. Impact reports a request-imposed
  node or hop cutoff first, then the fixture's scripted cutoff, then a result
  limit. Graph reports the smallest node cap it hits, with the node budget
  winning a tie, or a hop cutoff when the cap is sufficient.
- The contract defines neither an expected schema version on requests nor a
  schema-mismatch error. [schema-mismatch.json](errors/schema-mismatch.json)
  is a negative response fixture used to verify version validation.
