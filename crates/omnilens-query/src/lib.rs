//! # omnilens-query
//!
//! OmniQL — a query language for semantic code analysis.
//!
//! Grammar:
//! ```text
//! query     := FIND target (WHERE condition (AND condition)*)?
//! target    := "functions" | "types" | "modules" | "bindings" | "all"
//! condition := predicate | comparison | NOT condition
//! predicate := IDENT "(" args ")"
//! comparison:= field OP value
//! field     := "name" | "visibility" | "complexity" | "async" | "params" | "file"
//! OP        := "=" | "!=" | ">" | "<" | ">=" | "<=" | "~" (regex)
//! value     := STRING | NUMBER | BOOL | IDENT
//! ```

pub mod parser;
pub mod executor;

use omnilens_graph::SemanticGraph;

/// A query result — matching nodes with metadata.
pub struct QueryResult {
    pub matches: Vec<QueryMatch>,
    pub total_scanned: usize,
    pub query_text: String,
}

pub struct QueryMatch {
    pub node_id: omnilens_ir::NodeId,
    pub file: String,
    pub line: u32,
    pub name: String,
    pub kind: String,
    pub description: String,
}

/// Parse and execute an OmniQL query.
pub fn run_query(graph: &SemanticGraph, query_str: &str) -> anyhow::Result<QueryResult> {
    let ast = parser::parse(query_str)?;
    executor::execute(graph, &ast, query_str)
}
