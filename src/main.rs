use clap::Parser;

mod http;
mod crawler;

#[derive(Parser)]
struct Args {
    #[arg(short = 's', default_value = "fakebook.khoury.northeastern.edu")]
    server: String,

    #[arg(short = 'p', default_value_t = 443)]
    port: u32,

    username: String,

    password: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
}
