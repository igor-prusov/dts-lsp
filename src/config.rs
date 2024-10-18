use clap::Parser;
use std::sync::OnceLock;

#[derive(clap::Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Enable experimental features
    #[arg(long)]
    experimental: bool,
}

#[derive(Debug)]
pub struct Config {
    #[allow(dead_code)]
    pub experimental: bool,
    pub process_neighbours: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            experimental: false,
            process_neighbours: true,
        }
    }
}

pub static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn get() -> &'static Config {
    CONFIG.get_or_init(|| Config {
        experimental: Args::parse().experimental,
        ..Default::default()
    })
}
