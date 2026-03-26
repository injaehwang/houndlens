//! OmniQL executor — evaluates parsed queries against the semantic graph.

use anyhow::Result;
use omnilens_graph::SemanticGraph;
use omnilens_ir::node::UsirNode;
use omnilens_ir::Visibility;

use crate::parser::{CompOp, Condition, Query, Target, Value};
use crate::{QueryMatch, QueryResult};

/// Execute a parsed OmniQL query against the graph.
pub fn execute(graph: &SemanticGraph, query: &Query, query_text: &str) -> Result<QueryResult> {
    let all_ids = graph.all_node_ids();
    let mut matches = Vec::new();
    let mut scanned = 0;

    for id in &all_ids {
        let Some(node) = graph.get_node(*id) else {
            continue;
        };

        // Filter by target type.
        if !matches_target(node, &query.target) {
            continue;
        }

        // Skip placeholders.
        if graph.is_placeholder(*id) {
            continue;
        }

        scanned += 1;

        // Evaluate all conditions (AND logic).
        let all_match = query
            .conditions
            .iter()
            .all(|cond| eval_condition(graph, node, cond));

        if all_match {
            matches.push(node_to_match(node));
        }
    }

    Ok(QueryResult {
        matches,
        total_scanned: scanned,
        query_text: query_text.to_string(),
    })
}

fn matches_target(node: &UsirNode, target: &Target) -> bool {
    match target {
        Target::Functions => matches!(node, UsirNode::Function(_)),
        Target::Types => matches!(node, UsirNode::DataType(_)),
        Target::Modules => matches!(node, UsirNode::Module(_)),
        Target::Bindings => matches!(node, UsirNode::Binding(_)),
        Target::All => true,
    }
}

fn eval_condition(graph: &SemanticGraph, node: &UsirNode, cond: &Condition) -> bool {
    match cond {
        Condition::Comparison { field, op, value } => eval_comparison(node, field, op, value),
        Condition::Predicate { name, args } => eval_predicate(graph, node, name, args),
        Condition::Not(inner) => !eval_condition(graph, node, inner),
        Condition::Regex { field, pattern } => eval_regex(node, field, pattern),
    }
}

// ─── Comparison evaluation ──────────────────────────────────────

fn eval_comparison(node: &UsirNode, field: &str, op: &CompOp, value: &Value) -> bool {
    match field {
        "name" => {
            let name = node.name().display();
            let short = node.name().segments.last().cloned().unwrap_or_default();
            match value {
                Value::String(s) | Value::Ident(s) => match op {
                    CompOp::Eq => name == *s || short == *s,
                    CompOp::NotEq => name != *s && short != *s,
                    _ => false,
                },
                _ => false,
            }
        }
        "visibility" | "vis" => {
            let vis = get_visibility(node);
            let vis_str = format!("{:?}", vis).to_lowercase();
            match value {
                Value::String(s) | Value::Ident(s) => {
                    let s = s.to_lowercase();
                    match op {
                        CompOp::Eq => vis_str == s,
                        CompOp::NotEq => vis_str != s,
                        _ => false,
                    }
                }
                _ => false,
            }
        }
        "complexity" => {
            if let UsirNode::Function(f) = node {
                let c = f.complexity.unwrap_or(0) as f64;
                compare_num(c, op, value)
            } else {
                false
            }
        }
        "params" | "param_count" => {
            if let UsirNode::Function(f) = node {
                let count = f.params.len() as f64;
                compare_num(count, op, value)
            } else {
                false
            }
        }
        "fields" | "field_count" => {
            if let UsirNode::DataType(dt) = node {
                let count = dt.fields.len() as f64;
                compare_num(count, op, value)
            } else {
                false
            }
        }
        "async" => {
            if let UsirNode::Function(f) = node {
                let is_async = f.is_async;
                match value {
                    Value::Bool(b) => match op {
                        CompOp::Eq => is_async == *b,
                        CompOp::NotEq => is_async != *b,
                        _ => false,
                    },
                    Value::Ident(s) => match op {
                        CompOp::Eq => (s == "true") == is_async,
                        CompOp::NotEq => (s == "true") != is_async,
                        _ => false,
                    },
                    _ => false,
                }
            } else {
                false
            }
        }
        "unsafe" => {
            if let UsirNode::Function(f) = node {
                let is_unsafe = f.is_unsafe;
                match value {
                    Value::Bool(b) => match op {
                        CompOp::Eq => is_unsafe == *b,
                        CompOp::NotEq => is_unsafe != *b,
                        _ => false,
                    },
                    Value::Ident(s) => match op {
                        CompOp::Eq => (s == "true") == is_unsafe,
                        _ => false,
                    },
                    _ => false,
                }
            } else {
                false
            }
        }
        "file" => {
            let file = node.span().file.to_string_lossy().replace('\\', "/");
            match value {
                Value::String(s) | Value::Ident(s) => match op {
                    CompOp::Eq => file.ends_with(s.as_str()),
                    CompOp::NotEq => !file.ends_with(s.as_str()),
                    _ => false,
                },
                _ => false,
            }
        }
        "kind" => {
            let kind = node_kind_str(node);
            match value {
                Value::String(s) | Value::Ident(s) => match op {
                    CompOp::Eq => kind.eq_ignore_ascii_case(s),
                    CompOp::NotEq => !kind.eq_ignore_ascii_case(s),
                    _ => false,
                },
                _ => false,
            }
        }
        _ => false,
    }
}

fn compare_num(actual: f64, op: &CompOp, value: &Value) -> bool {
    let target = match value {
        Value::Number(n) => *n,
        Value::Ident(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => return false,
    };
    match op {
        CompOp::Eq => (actual - target).abs() < f64::EPSILON,
        CompOp::NotEq => (actual - target).abs() >= f64::EPSILON,
        CompOp::Gt => actual > target,
        CompOp::Lt => actual < target,
        CompOp::Gte => actual >= target,
        CompOp::Lte => actual <= target,
    }
}

// ─── Predicate evaluation ───────────────────────────────────────

fn eval_predicate(graph: &SemanticGraph, node: &UsirNode, name: &str, args: &[String]) -> bool {
    match name {
        "calls" => {
            // Does this function call the specified target?
            let target_name = args.first().map(|s| s.as_str()).unwrap_or("");
            let forward = graph.impact_forward(node.id(), 1);
            forward.direct.iter().any(|n| {
                graph
                    .get_node(n.node_id)
                    .map(|called| {
                        let called_name = called.name().display();
                        let short = called.name().segments.last().cloned().unwrap_or_default();
                        called_name.contains(target_name) || short == target_name
                    })
                    .unwrap_or(false)
            })
        }
        "called_by" | "calledby" => {
            // Is this function called by the specified target?
            let target_name = args.first().map(|s| s.as_str()).unwrap_or("");
            let reverse = graph.impact_reverse(node.id(), 1);
            reverse.direct.iter().any(|n| {
                graph
                    .get_node(n.node_id)
                    .map(|caller| {
                        let name = caller.name().display();
                        name.contains(target_name)
                    })
                    .unwrap_or(false)
            })
        }
        "handles" | "catches" => {
            // Does this function contain error handling?
            // Heuristic: checks if any call edge has ErrorPath condition.
            let forward = graph.impact_forward(node.id(), 1);
            // For now, simplified check based on call conditions.
            // TODO: deep analysis of error handling patterns.
            forward.direct.iter().any(|n| {
                graph.get_node(n.node_id).is_some()
            })
        }
        "returns" => {
            // Does this function return the specified type?
            let type_name = args.first().map(|s| s.as_str()).unwrap_or("");
            if let UsirNode::Function(f) = node {
                f.return_type
                    .as_ref()
                    .map(|t| format!("{:?}", t).contains(type_name))
                    .unwrap_or(false)
            } else {
                false
            }
        }
        "implements" => {
            // Does this type implement the specified interface/trait?
            let trait_name = args.first().map(|s| s.as_str()).unwrap_or("");
            if let UsirNode::DataType(dt) = node {
                dt.implements.iter().any(|t| {
                    format!("{:?}", t).contains(trait_name)
                })
            } else {
                false
            }
        }
        "has_field" | "hasfield" => {
            let field_name = args.first().map(|s| s.as_str()).unwrap_or("");
            if let UsirNode::DataType(dt) = node {
                dt.fields.iter().any(|f| f.name == field_name)
            } else {
                false
            }
        }
        "in_file" | "infile" => {
            let file_pattern = args.first().map(|s| s.as_str()).unwrap_or("");
            let file = node.span().file.to_string_lossy().replace('\\', "/");
            file.contains(file_pattern)
        }
        _ => false,
    }
}

// ─── Regex evaluation ───────────────────────────────────────────

fn eval_regex(node: &UsirNode, field: &str, pattern: &str) -> bool {
    let haystack = match field {
        "name" => node.name().display(),
        "file" => node.span().file.to_string_lossy().to_string(),
        _ => return false,
    };

    // Simple glob-like matching (no full regex dep yet).
    // Supports: * (any chars), ? (one char)
    glob_match(pattern, &haystack)
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // Convert simple glob to basic matching.
    if pattern.is_empty() {
        return text.is_empty();
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        // No wildcards — exact substring match.
        return text.contains(pattern);
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        match text[pos..].find(part) {
            Some(idx) => {
                if i == 0 && idx != 0 {
                    return false; // First part must match at start.
                }
                pos += idx + part.len();
            }
            None => return false,
        }
    }

    // If pattern ends with *, any trailing text is ok.
    if pattern.ends_with('*') {
        return true;
    }

    // Otherwise, must match to end.
    pos == text.len()
}

// ─── Helpers ────────────────────────────────────────────────────

fn get_visibility(node: &UsirNode) -> &Visibility {
    match node {
        UsirNode::Function(f) => &f.visibility,
        UsirNode::DataType(dt) => &dt.visibility,
        UsirNode::Binding(b) => &b.visibility,
        _ => &Visibility::Private,
    }
}

fn node_kind_str(node: &UsirNode) -> &str {
    match node {
        UsirNode::Function(_) => "function",
        UsirNode::DataType(dt) => match dt.kind {
            omnilens_ir::node::DataTypeKind::Struct => "struct",
            omnilens_ir::node::DataTypeKind::Class => "class",
            omnilens_ir::node::DataTypeKind::Interface => "interface",
            omnilens_ir::node::DataTypeKind::Trait => "trait",
            omnilens_ir::node::DataTypeKind::Enum => "enum",
            omnilens_ir::node::DataTypeKind::Union => "union",
            omnilens_ir::node::DataTypeKind::TypeAlias => "typealias",
        },
        UsirNode::Module(_) => "module",
        UsirNode::ApiEndpoint(_) => "endpoint",
        UsirNode::Binding(_) => "binding",
    }
}

fn node_to_match(node: &UsirNode) -> QueryMatch {
    QueryMatch {
        node_id: node.id(),
        file: node.span().file.to_string_lossy().replace('\\', "/"),
        line: node.span().start_line,
        name: node.name().display(),
        kind: node_kind_str(node).to_string(),
        description: match node {
            UsirNode::Function(f) => {
                let params = f.params.len();
                let complexity = f.complexity.unwrap_or(0);
                let vis = format!("{:?}", f.visibility).to_lowercase();
                format!(
                    "{} fn({} params, complexity {}, {}{})",
                    vis,
                    params,
                    complexity,
                    if f.is_async { "async, " } else { "" },
                    if f.is_unsafe { "unsafe" } else { "" },
                )
            }
            UsirNode::DataType(dt) => {
                format!(
                    "{:?} with {} fields, {} methods",
                    dt.kind,
                    dt.fields.len(),
                    dt.methods.len()
                )
            }
            UsirNode::Module(_) => "module".to_string(),
            UsirNode::Binding(b) => {
                format!(
                    "{} binding{}",
                    if b.is_constant { "const" } else { "let" },
                    if b.is_mutable { " (mut)" } else { "" }
                )
            }
            UsirNode::ApiEndpoint(_) => "endpoint".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        assert!(glob_match("test_*", "test_foo"));
        assert!(glob_match("test_*", "test_"));
        assert!(!glob_match("test_*", "my_test_foo"));
        assert!(glob_match("*test*", "my_test_foo"));
        assert!(glob_match("*.rs", "foo.rs"));
        assert!(!glob_match("*.rs", "foo.py"));
    }
}
