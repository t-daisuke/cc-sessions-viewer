mod models;
mod parser;
mod app;
mod ui;

use anyhow::Result;

fn main() -> Result<()> {
    app::run()
}
