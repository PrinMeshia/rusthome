//! Standalone binary — same server as `rusthome serve`.

use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "rusthome-web")]
#[command(about = "Minimal read-only rusthome dashboard (same as: rusthome serve)")]
struct Args {
    #[arg(long, default_value = "data", env = "RUSTHOME_DATA_DIR")]
    data_dir: PathBuf,
    #[arg(long, default_value = "127.0.0.1:8080")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    rusthome_web::run(args.data_dir, &args.bind, None).await
}
