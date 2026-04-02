use anyhow::Result;
use colored::Colorize;

pub fn run(query_str: &str) -> Result<()> {
    let mut engine = super::create_engine()?;

    // Index first.
    let idx = engine.index()?;
    if idx.files_analyzed > 0 {
        eprintln!(
            "{} {} files indexed",
            "Index".dimmed(),
            idx.files_analyzed
        );
    }

    let result = houndlens_query::run_query(&engine.graph, query_str)?;

    println!(
        "\n{} \"{}\"",
        "Query:".bold().cyan(),
        result.query_text,
    );
    println!(
        "{} {} matches (scanned {})\n",
        "Results:".bold().green(),
        result.matches.len(),
        result.total_scanned,
    );

    if result.matches.is_empty() {
        println!("  {}", "No matches found.".dimmed());
    } else {
        for m in &result.matches {
            let file_short = m.file.rsplit('/').next().unwrap_or(&m.file);
            println!(
                "  {} {}:{} — {} [{}]",
                "→".green(),
                file_short,
                m.line,
                m.name.bold(),
                m.description.dimmed(),
            );
        }
    }

    Ok(())
}
