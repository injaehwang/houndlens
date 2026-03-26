//! OmniQL parser — tokenizer and AST builder.

use anyhow::{bail, Context, Result};

// ─── AST ────────────────────────────────────────────────────────

/// Root AST node for an OmniQL query.
#[derive(Debug, Clone)]
pub struct Query {
    pub target: Target,
    pub conditions: Vec<Condition>,
}

/// What type of nodes to search for.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    Functions,
    Types,
    Modules,
    Bindings,
    All,
}

/// A filter condition.
#[derive(Debug, Clone)]
pub enum Condition {
    /// field OP value (e.g., `complexity > 10`)
    Comparison {
        field: String,
        op: CompOp,
        value: Value,
    },
    /// predicate(args) (e.g., `calls(db.query)`, `handles(Error)`)
    Predicate {
        name: String,
        args: Vec<String>,
    },
    /// NOT condition
    Not(Box<Condition>),
    /// name ~ "regex_pattern"
    Regex {
        field: String,
        pattern: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CompOp {
    Eq,
    NotEq,
    Gt,
    Lt,
    Gte,
    Lte,
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Number(f64),
    Bool(bool),
    Ident(String),
}

// ─── Tokenizer ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    // Keywords
    Find,
    Where,
    And,
    Or,
    Not,
    // Targets
    Ident(String),
    // Literals
    StringLit(String),
    NumberLit(f64),
    BoolLit(bool),
    // Operators
    Eq,        // =
    NotEq,     // !=
    Gt,        // >
    Lt,        // <
    Gte,       // >=
    Lte,       // <=
    Tilde,     // ~
    // Delimiters
    LParen,
    RParen,
    Comma,
    // End
    Eof,
}

fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\n' | '\r' => {
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            ',' => {
                tokens.push(Token::Comma);
                chars.next();
            }
            '~' => {
                tokens.push(Token::Tilde);
                chars.next();
            }
            '=' => {
                chars.next();
                tokens.push(Token::Eq);
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::NotEq);
                } else {
                    bail!("Expected '=' after '!'");
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Gte);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Lte);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '"' | '\'' => {
                let quote = ch;
                chars.next();
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == quote {
                        chars.next();
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                tokens.push(Token::StringLit(s));
            }
            c if c.is_ascii_digit() => {
                let mut num_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        num_str.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let num: f64 = num_str.parse().context("Invalid number")?;
                tokens.push(Token::NumberLit(num));
            }
            c if c.is_alphanumeric() || c == '_' || c == '.' || c == ':' || c == '*' => {
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '.' || c == ':' || c == '*' || c == '/' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let token = match ident.to_lowercase().as_str() {
                    "find" => Token::Find,
                    "where" => Token::Where,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    "true" => Token::BoolLit(true),
                    "false" => Token::BoolLit(false),
                    _ => Token::Ident(ident),
                };
                tokens.push(token);
            }
            _ => {
                bail!("Unexpected character: '{}'", ch);
            }
        }
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

// ─── Parser ─────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).cloned().unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.advance() {
            Token::Ident(s) => Ok(s),
            other => bail!("Expected identifier, got {:?}", other),
        }
    }

    fn parse_query(&mut self) -> Result<Query> {
        // Optional FIND keyword.
        if *self.peek() == Token::Find {
            self.advance();
        }

        // Target.
        let target = self.parse_target()?;

        // Optional WHERE clause.
        let mut conditions = Vec::new();
        if *self.peek() == Token::Where {
            self.advance();
            conditions = self.parse_conditions()?;
        }

        Ok(Query { target, conditions })
    }

    fn parse_target(&mut self) -> Result<Target> {
        let ident = self.expect_ident()?;
        match ident.to_lowercase().as_str() {
            "functions" | "function" | "fn" | "fns" | "methods" => Ok(Target::Functions),
            "types" | "type" | "structs" | "classes" | "interfaces" => Ok(Target::Types),
            "modules" | "module" | "mod" => Ok(Target::Modules),
            "bindings" | "binding" | "vars" | "constants" => Ok(Target::Bindings),
            "all" | "*" => Ok(Target::All),
            _ => bail!("Unknown target: '{}'. Use: functions, types, modules, bindings, all", ident),
        }
    }

    fn parse_conditions(&mut self) -> Result<Vec<Condition>> {
        let mut conditions = Vec::new();
        conditions.push(self.parse_condition()?);

        loop {
            match self.peek() {
                Token::And => {
                    self.advance();
                    conditions.push(self.parse_condition()?);
                }
                Token::Eof | Token::RParen => break,
                _ => break,
            }
        }

        Ok(conditions)
    }

    fn parse_condition(&mut self) -> Result<Condition> {
        // NOT condition
        if *self.peek() == Token::Not {
            self.advance();
            let inner = self.parse_condition()?;
            return Ok(Condition::Not(Box::new(inner)));
        }

        let ident = self.expect_ident()?;

        // Check if this is a predicate call: ident(args)
        if *self.peek() == Token::LParen {
            self.advance();
            let mut args = Vec::new();
            while *self.peek() != Token::RParen && *self.peek() != Token::Eof {
                match self.advance() {
                    Token::Ident(s) | Token::StringLit(s) => args.push(s),
                    Token::NumberLit(n) => args.push(n.to_string()),
                    Token::Comma => continue,
                    other => bail!("Unexpected token in predicate args: {:?}", other),
                }
            }
            if *self.peek() == Token::RParen {
                self.advance();
            }
            return Ok(Condition::Predicate {
                name: ident,
                args,
            });
        }

        // Comparison: field OP value
        let op = match self.advance() {
            Token::Eq => CompOp::Eq,
            Token::NotEq => CompOp::NotEq,
            Token::Gt => CompOp::Gt,
            Token::Lt => CompOp::Lt,
            Token::Gte => CompOp::Gte,
            Token::Lte => CompOp::Lte,
            Token::Tilde => {
                // Regex match
                let pattern = match self.advance() {
                    Token::StringLit(s) => s,
                    Token::Ident(s) => s,
                    other => bail!("Expected pattern after ~, got {:?}", other),
                };
                return Ok(Condition::Regex {
                    field: ident,
                    pattern,
                });
            }
            other => bail!("Expected operator after '{}', got {:?}", ident, other),
        };

        let value = match self.advance() {
            Token::StringLit(s) => Value::String(s),
            Token::NumberLit(n) => Value::Number(n),
            Token::BoolLit(b) => Value::Bool(b),
            Token::Ident(s) => {
                // Try to parse as number.
                if let Ok(n) = s.parse::<f64>() {
                    Value::Number(n)
                } else {
                    Value::Ident(s)
                }
            }
            other => bail!("Expected value, got {:?}", other),
        };

        Ok(Condition::Comparison {
            field: ident,
            op,
            value,
        })
    }
}

/// Parse an OmniQL query string into an AST.
pub fn parse(input: &str) -> Result<Query> {
    let tokens = tokenize(input)?;
    let mut parser = Parser::new(tokens);
    parser.parse_query()
}

// ─── Tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_query() {
        let q = parse("FIND functions WHERE complexity > 10").unwrap();
        assert_eq!(q.target, Target::Functions);
        assert_eq!(q.conditions.len(), 1);
    }

    #[test]
    fn test_predicate_query() {
        let q = parse("FIND functions WHERE calls(db.query) AND NOT handles(Error)").unwrap();
        assert_eq!(q.target, Target::Functions);
        assert_eq!(q.conditions.len(), 2);
        match &q.conditions[0] {
            Condition::Predicate { name, args } => {
                assert_eq!(name, "calls");
                assert_eq!(args, &["db.query"]);
            }
            _ => panic!("Expected predicate"),
        }
        match &q.conditions[1] {
            Condition::Not(inner) => {
                match inner.as_ref() {
                    Condition::Predicate { name, .. } => assert_eq!(name, "handles"),
                    _ => panic!("Expected predicate inside NOT"),
                }
            }
            _ => panic!("Expected NOT"),
        }
    }

    #[test]
    fn test_multi_condition() {
        let q = parse("FIND functions WHERE visibility = public AND async = true AND complexity > 5").unwrap();
        assert_eq!(q.conditions.len(), 3);
    }

    #[test]
    fn test_regex_query() {
        let q = parse(r#"FIND functions WHERE name ~ "test_.*""#).unwrap();
        assert_eq!(q.conditions.len(), 1);
        match &q.conditions[0] {
            Condition::Regex { field, pattern } => {
                assert_eq!(field, "name");
                assert_eq!(pattern, "test_.*");
            }
            _ => panic!("Expected regex condition"),
        }
    }

    #[test]
    fn test_types_query() {
        let q = parse("FIND types WHERE fields > 5").unwrap();
        assert_eq!(q.target, Target::Types);
    }

    #[test]
    fn test_no_where() {
        let q = parse("FIND functions").unwrap();
        assert_eq!(q.target, Target::Functions);
        assert!(q.conditions.is_empty());
    }

    #[test]
    fn test_lowercase() {
        let q = parse("find fns where complexity > 10").unwrap();
        assert_eq!(q.target, Target::Functions);
    }
}
