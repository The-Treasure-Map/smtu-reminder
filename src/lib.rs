mod app;
mod bot;
mod db;
mod formatting;
pub mod parser;
mod schedule_repo;
mod state;
pub mod structures;
mod tasks;

pub async fn run() -> anyhow::Result<()> {
    app::run().await
}
