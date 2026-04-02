use anyhow::Result;
use colored::Colorize;

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    println!("{} houndlens in {}", "Initializing".green().bold(), cwd.display());

    let mut engine = super::create_engine()?;

    // Create .houndlens directory.
    let houndlens_dir = cwd.join(".houndlens");
    std::fs::create_dir_all(&houndlens_dir)?;

    // Index and generate manifest.
    let idx = engine.index()?;

    let manifest = houndlens_core::manifest::generate(&cwd, &engine.graph);
    houndlens_core::manifest::write(&cwd, &manifest)?;

    println!(
        "{} {} files indexed, manifest generated",
        "Done.".green().bold(),
        idx.files_analyzed,
    );
    println!(
        "  {} .houndlens/manifest.json — AI agents will discover this automatically",
        "→".cyan()
    );
    println!(
        "  {} Run {} to scan for problems",
        "→".cyan(),
        "houndlens check".cyan()
    );
    Ok(())
}
