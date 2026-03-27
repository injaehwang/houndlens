//! Syntax validation — checks changed files for syntax errors using tree-sitter.
//!
//! This catches:
//! - Broken HTML/Vue tags
//! - Unclosed brackets/parens
//! - Malformed attributes
//! - Invalid Python/TypeScript/Rust syntax
//! - Indentation issues (Python)

use std::path::Path;

use crate::frontend::LanguageFrontend;

/// A syntax error found in a file.
pub struct SyntaxError {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub message: String,
    pub severity: SyntaxSeverity,
}

pub enum SyntaxSeverity {
    Error,
    Warning,
}

/// Check files for syntax errors using tree-sitter.
pub fn check_syntax(
    files: &[String],
    frontends: &[Box<dyn LanguageFrontend>],
) -> Vec<SyntaxError> {
    let mut errors = Vec::new();

    for file_path in files {
        let path = Path::new(file_path);

        let content = match std::fs::read(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        // Find the right frontend.
        let frontend = frontends
            .iter()
            .find(|f| f.extensions().contains(&ext));

        if let Some(fe) = frontend {
            // Parse and check for errors.
            match fe.parse_file(path, &content) {
                Ok(_) => {
                    // tree-sitter parsed OK, but check raw content for common issues.
                    check_common_issues(file_path, &content, ext, &mut errors);
                }
                Err(e) => {
                    errors.push(SyntaxError {
                        file: file_path.clone(),
                        line: 0,
                        col: 0,
                        message: format!("Parse error: {}", e),
                        severity: SyntaxSeverity::Error,
                    });
                }
            }
        }

        // Vue/HTML files — check with dedicated parser.
        if ext == "vue" || ext == "html" {
            check_vue_syntax(file_path, &content, &mut errors);
        }
    }

    errors
}

/// Check for common code issues across all languages.
fn check_common_issues(file: &str, content: &[u8], ext: &str, errors: &mut Vec<SyntaxError>) {
    let text = String::from_utf8_lossy(content);

    // 1. Unmatched brackets/parens/braces.
    check_bracket_balance(file, &text, errors);

    // 2. Trailing whitespace on lines (warning).
    // Skip — too noisy.

    // 3. Mixed tabs and spaces (Python = error, others = warning).
    if ext == "py" {
        check_python_indentation(file, &text, errors);
    }

    // 4. Console.log / print statements left in (warning).
    // Skip — that's a lint rule, not syntax.
}

/// Check bracket/paren/brace balance.
fn check_bracket_balance(file: &str, text: &str, errors: &mut Vec<SyntaxError>) {
    let mut stack: Vec<(char, u32, u32)> = Vec::new(); // (char, line, col)
    let mut in_string = false;
    let mut string_char: char = '"';
    let mut escaped = false;
    let mut line: u32 = 1;
    let mut col: u32 = 0;
    let mut in_comment = false;
    let mut in_line_comment = false;
    let chars: Vec<char> = text.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        col += 1;

        if ch == '\n' {
            line += 1;
            col = 0;
            in_line_comment = false;
            i += 1;
            continue;
        }

        // Skip comments.
        if in_line_comment {
            i += 1;
            continue;
        }
        if in_comment {
            if ch == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                in_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if !in_string && ch == '/' && i + 1 < chars.len() {
            if chars[i + 1] == '/' {
                in_line_comment = true;
                i += 2;
                continue;
            }
            if chars[i + 1] == '*' {
                in_comment = true;
                i += 2;
                continue;
            }
        }
        // Skip Python comments.
        if !in_string && ch == '#' {
            in_line_comment = true;
            i += 1;
            continue;
        }

        // String handling.
        if escaped {
            escaped = false;
            i += 1;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            i += 1;
            continue;
        }
        if (ch == '"' || ch == '\'' || ch == '`') && !in_string {
            in_string = true;
            string_char = ch;
            i += 1;
            continue;
        }
        if in_string && ch == string_char {
            in_string = false;
            i += 1;
            continue;
        }
        if in_string {
            i += 1;
            continue;
        }

        // Bracket matching.
        match ch {
            '(' | '[' | '{' => stack.push((ch, line, col)),
            ')' | ']' | '}' => {
                let expected = match ch {
                    ')' => '(',
                    ']' => '[',
                    '}' => '{',
                    _ => unreachable!(),
                };
                match stack.pop() {
                    Some((open, _, _)) if open == expected => {}
                    Some((open, open_line, open_col)) => {
                        errors.push(SyntaxError {
                            file: file.to_string(),
                            line,
                            col,
                            message: format!(
                                "Mismatched bracket: '{}' at line {} closes '{}' from line {}:{}",
                                ch, line, open, open_line, open_col
                            ),
                            severity: SyntaxSeverity::Error,
                        });
                    }
                    None => {
                        errors.push(SyntaxError {
                            file: file.to_string(),
                            line,
                            col,
                            message: format!("Unexpected closing '{}' with no matching opener", ch),
                            severity: SyntaxSeverity::Error,
                        });
                    }
                }
            }
            _ => {}
        }

        i += 1;
    }

    // Unclosed brackets.
    for (ch, open_line, open_col) in &stack {
        errors.push(SyntaxError {
            file: file.to_string(),
            line: *open_line,
            col: *open_col,
            message: format!("Unclosed '{}' at line {}:{}", ch, open_line, open_col),
            severity: SyntaxSeverity::Error,
        });
    }
}

/// Check Python indentation consistency.
fn check_python_indentation(file: &str, text: &str, errors: &mut Vec<SyntaxError>) {
    let mut uses_tabs = false;
    let mut uses_spaces = false;

    for (i, line) in text.lines().enumerate() {
        if line.is_empty() || line.trim().is_empty() {
            continue;
        }
        let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
        if indent.contains('\t') {
            uses_tabs = true;
        }
        if indent.contains(' ') && !indent.contains('\t') {
            uses_spaces = true;
        }

        // Mixed tabs and spaces on same line.
        if indent.contains('\t') && indent.contains(' ') {
            errors.push(SyntaxError {
                file: file.to_string(),
                line: (i + 1) as u32,
                col: 1,
                message: "Mixed tabs and spaces in indentation".to_string(),
                severity: SyntaxSeverity::Error,
            });
        }
    }

    if uses_tabs && uses_spaces {
        errors.push(SyntaxError {
            file: file.to_string(),
            line: 0,
            col: 0,
            message: "File mixes tab and space indentation".to_string(),
            severity: SyntaxSeverity::Warning,
        });
    }
}

/// Check Vue/HTML syntax.
fn check_vue_syntax(file: &str, content: &[u8], errors: &mut Vec<SyntaxError>) {
    let text = String::from_utf8_lossy(content);

    // 1. Check HTML tag balance.
    check_html_tags(file, &text, errors);

    // 2. Check for malformed attributes.
    check_html_attributes(file, &text, errors);
}

/// Check HTML/Vue tag balance.
fn check_html_tags(file: &str, text: &str, errors: &mut Vec<SyntaxError>) {
    let mut stack: Vec<(String, u32)> = Vec::new();

    // Self-closing tags that don't need a closing tag.
    let void_tags = [
        "br", "hr", "img", "input", "meta", "link", "area", "base",
        "col", "embed", "source", "track", "wbr",
    ];

    for (line_num, line) in text.lines().enumerate() {
        let line_n = (line_num + 1) as u32;
        let trimmed = line.trim();

        // Opening tags.
        let mut pos = 0;
        while let Some(start) = trimmed[pos..].find('<') {
            let abs_start = pos + start;
            let rest = &trimmed[abs_start..];

            // Skip comments, closing tags, doctypes.
            if rest.starts_with("<!--") || rest.starts_with("<!") {
                break;
            }
            if rest.starts_with("</") {
                // Closing tag.
                if let Some(end) = rest.find('>') {
                    let tag = rest[2..end].trim().to_lowercase();
                    if !tag.is_empty() {
                        match stack.last() {
                            Some((open_tag, _)) if open_tag == &tag => {
                                stack.pop();
                            }
                            Some((open_tag, open_line)) => {
                                // Check if it's somewhere deeper in the stack.
                                let found = stack.iter().rposition(|(t, _)| t == &tag);
                                if let Some(idx) = found {
                                    // Pop everything above it — these are unclosed.
                                    while stack.len() > idx {
                                        let (unclosed_tag, unclosed_line) = stack.pop().unwrap();
                                        if unclosed_tag != tag {
                                            errors.push(SyntaxError {
                                                file: file.to_string(),
                                                line: unclosed_line,
                                                col: 0,
                                                message: format!("Unclosed <{}>", unclosed_tag),
                                                severity: SyntaxSeverity::Error,
                                            });
                                        }
                                    }
                                }
                            }
                            None => {}
                        }
                    }
                    pos = abs_start + end + 1;
                    continue;
                }
            }

            // Opening tag.
            if rest.starts_with('<') && !rest.starts_with("</") {
                // Extract tag name.
                let tag_rest = &rest[1..];
                let tag_end = tag_rest.find(|c: char| c.is_whitespace() || c == '>' || c == '/');
                if let Some(end) = tag_end {
                    let tag = tag_rest[..end].to_lowercase();
                    if !tag.is_empty() && tag.chars().next().map_or(false, |c| c.is_alphabetic()) {
                        // Check if self-closing.
                        let full_tag_end = rest.find('>').unwrap_or(rest.len());
                        let is_self_closing = rest[..full_tag_end].ends_with('/')
                            || void_tags.contains(&tag.as_str());

                        if !is_self_closing && !void_tags.contains(&tag.as_str()) {
                            // Skip template/script/style at root level of .vue files.
                            if !matches!(tag.as_str(), "template" | "script" | "style") || stack.len() > 0 {
                                stack.push((tag, line_n));
                            }
                        }

                        pos = abs_start + full_tag_end + 1;
                        continue;
                    }
                }
            }

            pos = abs_start + 1;
        }
    }
}

/// Check for malformed HTML attributes.
fn check_html_attributes(file: &str, text: &str, errors: &mut Vec<SyntaxError>) {
    for (line_num, line) in text.lines().enumerate() {
        let line_n = (line_num + 1) as u32;
        let trimmed = line.trim();

        // Check for `/` not followed by `>` in a tag context (like `/ show-empty>`).
        if trimmed.contains("/ ") && !trimmed.starts_with("//") && !trimmed.starts_with("*") {
            // In an HTML-like context?
            if trimmed.contains('>') || trimmed.contains('<') {
                let pos = trimmed.find("/ ");
                if let Some(p) = pos {
                    // Make sure it's not in a string or comment.
                    let before = &trimmed[..p];
                    if !before.contains("//") && !before.contains("<!--") {
                        errors.push(SyntaxError {
                            file: file.to_string(),
                            line: line_n,
                            col: (p + 1) as u32,
                            message: "Malformed tag: '/' followed by space — did you mean '/>' (self-closing)?".to_string(),
                            severity: SyntaxSeverity::Error,
                        });
                    }
                }
            }
        }

        // Check for `="` without a preceding attribute name.
        if trimmed.contains(" =\"") || trimmed.starts_with("=\"") {
            errors.push(SyntaxError {
                file: file.to_string(),
                line: line_n,
                col: 0,
                message: "Attribute value without attribute name".to_string(),
                severity: SyntaxSeverity::Error,
            });
        }
    }
}
