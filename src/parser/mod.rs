/// M language parser module.
pub mod ast;
pub mod grammar;
pub mod lexer;

use ast::MDocument;
use grammar::{ParseError, Parser};
use lexer::tokenize;

/// Parse M source code into an AST document.
pub fn parse(input: &str) -> Result<MDocument, ParseError> {
    let tokens = tokenize(input).map_err(|e| ParseError {
        line: e.line,
        col: e.col,
        message: e.message,
    })?;
    let mut parser = Parser::new(tokens);
    parser.parse_document()
}

/// Parse a TMDL file and extract M expressions with their table names.
pub fn parse_tmdl(input: &str) -> Result<Vec<(String, MDocument)>, ParseError> {
    let mut results = Vec::new();
    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;
    let mut current_table = String::new();

    while i < lines.len() {
        let trimmed = lines[i].trim();

        // Look for table declaration
        if trimmed.starts_with("table ") {
            current_table = trimmed
                .strip_prefix("table ")
                .unwrap_or("")
                .trim()
                .to_string();
        }

        // Look for source = ``` block (backtick-delimited)
        if trimmed.starts_with("source") && trimmed.contains("```") {
            // Collect M code until closing ```
            let mut m_code = String::new();
            i += 1;
            while i < lines.len() {
                let line = lines[i];
                let line_trimmed = line.trim();
                if line_trimmed == "```" {
                    break;
                }
                if !m_code.is_empty() {
                    m_code.push('\n');
                }
                m_code.push_str(line_trimmed);
                i += 1;
            }

            if !m_code.is_empty() {
                let table_name = if current_table.is_empty() {
                    "query".to_string()
                } else {
                    current_table.clone()
                };
                let doc = parse(&m_code)?;
                results.push((table_name, doc));
            }
        }
        // Look for indentation-based source block: "source =" or "source=\n" followed by indented M code
        else if trimmed.starts_with("source") && trimmed.ends_with('=') {
            let source_indent = indent_level(lines[i]);
            let mut m_code = String::new();
            i += 1;

            // Collect lines that are indented deeper than the source line
            while i < lines.len() {
                let line = lines[i];
                // Empty lines within the block are preserved
                if line.trim().is_empty() {
                    if !m_code.is_empty() {
                        m_code.push('\n');
                    }
                    i += 1;
                    continue;
                }
                let line_indent = indent_level(line);
                if line_indent <= source_indent {
                    break;
                }
                if !m_code.is_empty() {
                    m_code.push('\n');
                }
                m_code.push_str(line.trim());
                i += 1;
            }

            if !m_code.is_empty() {
                let table_name = if current_table.is_empty() {
                    "query".to_string()
                } else {
                    current_table.clone()
                };
                let doc = parse(&m_code)?;
                results.push((table_name, doc));
            }
            // Don't increment i again — we already advanced past the block
            continue;
        }

        i += 1;
    }

    Ok(results)
}

/// Count leading whitespace characters (tabs count as 1).
fn indent_level(line: &str) -> usize {
    line.len() - line.trim_start().len()
}
