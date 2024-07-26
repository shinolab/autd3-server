#![allow(non_snake_case)]

mod log_formatter;

use log_formatter::LogFormatter;

use autd3_link_twincat::TwinCAT;

use autd3_protobuf::{lightweight::LightweightServer, *};

use tokio::{runtime::Runtime, sync::mpsc};
use tonic::transport::Server;

use clap::Parser;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(
    help_template = "Author: {author-with-newline} {about-section}Version: {version} \n\n {usage-heading} {usage} \n\n {all-args} {tab}"
)]
struct Arg {
    /// Client port
    #[clap(short = 'p', long = "port")]
    port: u16,
}

async fn main_() -> anyhow::Result<()> {
    let arg = Arg::parse();

    let port = arg.port;

    let server = LightweightServer::new(TwinCAT::builder);

    let (tx, mut rx) = mpsc::channel(1);
    ctrlc::set_handler(move || {
        let rt = Runtime::new().expect("failed to obtain a new Runtime object");
        rt.block_on(tx.send(())).unwrap();
    })
    .expect("Error setting Ctrl-C handler");

    let addr = format!("0.0.0.0:{}", port).parse()?;
    tracing::info!("Waiting for client connection on {}", addr);

    Server::builder()
        .add_service(ecat_light_server::EcatLightServer::new(server))
        .serve_with_shutdown(addr, async {
            let _ = rx.recv().await;
        })
        .await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().event_format(LogFormatter).init();

    match main_().await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("{}", e);
            std::process::exit(-1);
        }
    }
}
