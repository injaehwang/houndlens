use anyhow::Result;
use colored::Colorize;

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    println!("{} omnilens in {}", "Initializing".green().bold(), cwd.display());

    let mut engine = super::create_engine()?;

    // Create .omnilens directory.
    let omnilens_dir = cwd.join(".omnilens");
    std::fs::create_dir_all(&omnilens_dir)?;

    // Index and generate manifest.
    let idx = engine.index()?;

    let manifest = omnilens_core::manifest::generate(&cwd, &engine.graph);
    omnilens_core::manifest::write(&cwd, &manifest)?;

    println!(
        "{} {} files indexed, manifest generated",
        "Done.".green().bold(),
        idx.files_analyzed,
    );
    println!(
        "  {} .omnilens/manifest.json — AI agents will discover this automatically",
        "→".cyan()
    );
    println!(
        "  {} Run {} to scan for problems",
        "→".cyan(),
        "omnilens check".cyan()
    );
    Ok(())
}
