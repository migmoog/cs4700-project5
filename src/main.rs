use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector,
    rustls::{ClientConfig, RootCertStore, pki_types::ServerName},
};

use crate::http::{Method, RequestBuilder};

mod crawler;
mod http;

#[derive(Parser)]
struct Args {
    #[arg(short = 's', default_value = "fakebook.khoury.northeastern.edu")]
    server: String,

    #[arg(short = 'p', default_value_t = 443)]
    port: u32,

    username: String,

    password: String,
}

async fn start<T>(mut socket: T, host: &str) -> Result<()>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let login_res = RequestBuilder::new(Method::Get, "/accounts/login/".to_string())
        .header("Host", host)
        .send(&mut socket).await?;
    eprintln!("{:#?}", login_res);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let host_and_port = format!("{}:{}", args.server, args.port);
    if args.port == 443 {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let socket = TcpStream::connect(&host_and_port).await?;
        let server_name = ServerName::try_from(args.server.clone())?;
        let socket = connector.connect(server_name, socket).await?;
        start(socket, args.server.as_str()).await?;
    } else {
        let socket = TcpStream::connect(&host_and_port).await?;
        start(socket, &args.server).await?;
    }

    Ok(())
}
