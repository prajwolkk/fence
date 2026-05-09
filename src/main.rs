//! Fence CLI binary entrypoint.

use std::error::Error;

mod cli;
mod serve;
mod tui;

fn main() -> Result<(), Box<dyn Error>> {
    cli::run()
}
