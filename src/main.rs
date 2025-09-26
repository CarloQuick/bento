extern crate dotenv;
mod cli;
mod runtime;
use dotenv::dotenv;

fn main() {
    dotenv().ok();
    cli::bento_cli();
}
