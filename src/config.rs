use clap::Parser;
use std::sync::OnceLock;

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Enable experimental features
    #[arg(long)]
    experimental: bool,

    /// Scan all files in project instead of using heuristics
    /// (High memory and CPU usage!)
    #[cfg(feature = "walkdir")]
    #[arg(long)]
    full_scan: bool,
}

#[derive(Debug)]
pub struct Config {
    #[allow(dead_code)]
    pub experimental: bool,
    pub process_neighbours: bool,
    #[allow(dead_code)]
    pub full_scan: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            experimental: false,
            process_neighbours: true,
            full_scan: false,
        }
    }
}

pub static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get() -> &'static Config {
    let args = Args::parse();
    CONFIG.get_or_init(|| Config {
        experimental: args.experimental,
        #[cfg(feature = "walkdir")]
        full_scan: args.full_scan,
        ..Default::default()
    })
}
