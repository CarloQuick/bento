extern crate dotenv;
mod runtime;
use dotenv::dotenv;

fn main() {
    dotenv().ok();
    runtime::create_namespace();
}
