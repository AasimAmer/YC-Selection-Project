//! Directed symbol relations for a single graph snapshot.
use faultline_contract::{FileId, GraphNode, PartialResultReason, SymbolId, SymbolKind};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write;

/// Creates an ID from lexical identity. Use a normalized repository-relative
/// file ID and an ordinal among otherwise indistinguishable siblings.
pub fn symbol_id(
    file: &FileId,
    qualified_name: &[&str],
    kind: &SymbolKind,
    ordinal: u32,
) -> SymbolId {
    let kind = match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class => "class",
        SymbolKind::Method => "method",
        SymbolKind::Variable => "variable",
        SymbolKind::Interface => "interface",
        SymbolKind::Type => "type",
        SymbolKind::Module => "module",
    };
    // Length-prefix each component so delimiters inside names cannot collide.
    let mut identity = format!("{kind}:{ordinal}:{}:{}", file.0.len(), file.0);
    for name in qualified_name {
        write!(identity, "{}:{name}", name.len()).expect("writing to a String cannot fail");
    }
    SymbolId(identity)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraphError {
    DuplicateNode,
    UnknownNode,
}

/// Stores resolved consumer → dependency relations. Parallel call sites
/// share one adjacency entry. Source evidence is stored separately by the index.
#[derive(Debug, Default)]
pub struct DependencyGraph {
    nodes: BTreeMap<String, GraphNode>,
    outgoing: BTreeMap<String, BTreeSet<String>>,
    incoming: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, PartialEq)]
pub struct ReverseVisit<'a> {
    pub consumer: &'a GraphNode,
    /// Next node on this shortest path from the consumer back to the target.
    pub dependency: &'a SymbolId,
    pub hops: u32,
}

#[derive(Debug, PartialEq)]
pub struct ReverseTraversal<'a> {
    /// Discovery order is breadth-first, with neighbors ordered by symbol ID.
    pub visits: Vec<ReverseVisit<'a>>,
    pub partial_reason: Option<PartialResultReason>,
}

impl DependencyGraph {
    pub fn insert_node(&mut self, node: GraphNode) -> Result<(), GraphError> {
        let std::collections::btree_map::Entry::Vacant(entry) = self.nodes.entry(node.id.0.clone())
        else {
            return Err(GraphError::DuplicateNode);
        };
        entry.insert(node);
        Ok(())
    }

    pub fn insert_relation(
        &mut self,
        consumer: &SymbolId,
        dependency: &SymbolId,
    ) -> Result<(), GraphError> {
        self.node(consumer)?;
        self.node(dependency)?;
        self.outgoing
            .entry(consumer.0.clone())
            .or_default()
            .insert(dependency.0.clone());
        self.incoming
            .entry(dependency.0.clone())
            .or_default()
            .insert(consumer.0.clone());
        Ok(())
    }

    pub fn node(&self, id: &SymbolId) -> Result<&GraphNode, GraphError> {
        self.nodes.get(&id.0).ok_or(GraphError::UnknownNode)
    }

    pub fn dependencies(
        &self,
        consumer: &SymbolId,
    ) -> Result<impl Iterator<Item = &GraphNode>, GraphError> {
        self.node(consumer)?;
        Ok(self
            .outgoing
            .get(&consumer.0)
            .into_iter()
            .flatten()
            .map(|id| &self.nodes[id]))
    }

    /// Traverses incoming relations. The node budget counts consumers and
    /// excludes the target. A limit is partial only if a consumer was omitted.
    pub fn reverse(
        &self,
        target: &SymbolId,
        max_hops: u32,
        node_budget: usize,
    ) -> Result<ReverseTraversal<'_>, GraphError> {
        let target = self.node(target)?;
        let mut seen = BTreeSet::from([target.id.0.as_str()]);
        let mut pending = VecDeque::from([(target, 0)]);
        let mut traversal = ReverseTraversal {
            visits: Vec::new(),
            partial_reason: None,
        };
        while let Some((dependency, hops)) = pending.pop_front() {
            for id in self.incoming.get(&dependency.id.0).into_iter().flatten() {
                if seen.contains(id.as_str()) {
                    continue;
                }
                if hops == max_hops {
                    traversal.partial_reason = Some(PartialResultReason::HopLimit);
                    return Ok(traversal);
                }
                if traversal.visits.len() == node_budget {
                    traversal.partial_reason = Some(PartialResultReason::NodeBudget);
                    return Ok(traversal);
                }
                seen.insert(id.as_str());
                let consumer = &self.nodes[id];
                traversal.visits.push(ReverseVisit {
                    consumer,
                    dependency: &dependency.id,
                    hops: hops + 1,
                });
                pending.push_back((consumer, hops + 1));
            }
        }
        Ok(traversal)
    }
}
