mod greet;

use clap::Parser;
use lab_core::Record;

#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "world")]
    name: String,
    #[arg(long, default_value_t = 1)]
    id: u32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = Args::parse();
    let record = Record::new(args.id, &args.name);
    println!("{}", greet::line(&record));
}
// lab leaf 1794558554-4
