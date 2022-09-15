use clap::{ Command };
use std::{ env, sync::Arc, time::Duration };
mod error;
mod crawler;
mod interfaces;
mod cve;

use crate::crawler::Crawler;
use crate::cve::{ CveDetail };

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Command::new(clap::crate_name!())
        .version(clap::crate_version!())
        .about(clap::crate_description!())
        .subcommand(
            Command::new("run").about("Run a spider")
        )
        .arg_required_else_help(true)
        .get_matches();

    env::set_var("RUST_LOG", "info,crawler=debug");
    env_logger::init();

    if let Some(matches) = cli.subcommand_matches("run") {
        let crawler = Crawler::new(Duration::from_millis(200), 2, 500);
        let spider = Arc::new(CveDetail::new());
        crawler.run(spider);
    }else {
        println!("not command !");
    }

    Ok(())
}
