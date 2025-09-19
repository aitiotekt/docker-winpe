//! web mode: Open browser to web UI.

pub fn run(base_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("{}/ui/", base_url);

    eprintln!("Opening browser to: {}", url);

    open::that(&url)?;

    Ok(())
}
