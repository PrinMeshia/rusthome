//! Standalone binary — same server as `rusthome serve`.

use std::path::PathBuf;

use clap::Parser;
use rusthome_app::rusthome_file::load_rusthome_file;

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
    let zigbee2mqtt = load_rusthome_file(&args.data_dir)?.zigbee2mqtt;
    rusthome_web::run(args.data_dir, &args.bind, None, None, zigbee2mqtt, None).await
}
