use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "migrate", about = "rust-brain v2 migration runner (RUSAA-19/20)")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Debug)]
enum Command {
    /// Apply control-plane `PostgreSQL` migrations
    Pg {
        #[arg(long)]
        control: bool,
        #[arg(long)]
        all_tenants: bool,
    },
    /// Create/update Kafka topics from infra/kafka/topics.yaml
    Kafka,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Pg { .. } => {
            eprintln!("migrate-pg: implementation in RUSAA-19");
        }
        Command::Kafka => {
            eprintln!("migrate-kafka: implementation in RUSAA-20");
        }
    }
    Ok(())
}
