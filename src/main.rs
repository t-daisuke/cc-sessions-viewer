mod models;
mod parser;
mod app;
mod ui;
pub mod index;
pub mod indexer;

use anyhow::Result;

fn main() -> Result<()> {
    app::run()
}
