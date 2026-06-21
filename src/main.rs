use std::sync::Arc;

use anyhow::{Result, anyhow};
use clap::Parser;
use scraper::Selector;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_rustls::{
    TlsConnector, TlsStream,
    rustls::{ClientConfig, RootCertStore, pki_types::ServerName},
};

use crate::{
    crawler::Crawler,
    http::{Method, RequestBuilder},
};

mod crawler;
mod http;

#[derive(Parser)]
struct P5Args {
    #[arg(short = 's', default_value = "fakebook.khoury.northeastern.edu")]
    server: String,

    #[arg(short = 'p', default_value_t = 443)]
    port: u32,

    username: String,

    password: String,
}

async fn start<T>(mut socket: T, args: &P5Args, is_tls: bool) -> Result<()>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let login_res = RequestBuilder::new(Method::Get, "/accounts/login/".to_string())
        .header("Host", &args.server)
        .send(&mut socket)
        .await?;
    // eprintln!("{:#?}", login_res);
    let login_page = login_res.html_body()?;

    let selector = Selector::parse("input[name=\"csrfmiddlewaretoken\"]")
        .map_err(|e| anyhow!("Selector error: {e}"))?;

    let Some(csrfmiddlewaretoken) = login_page
        .select(&selector)
        .next()
        .and_then(|e| e.value().attr("value"))
    else {
        return Err(anyhow!(
            "Couldn't find a csrfmiddlewaretoken to login: {login_page:#?}"
        ));
    };

    let cookies = login_res.cookies();
    let logged_in_res = RequestBuilder::new(Method::Post, "/accounts/login".to_string())
        .header("Host", &args.server)
        .header(
            "Referer",
            &format!("https://{}/accounts/login/", args.server),
        )
        .header("Cookie", &cookies)
        .form_item("username", &args.username)
        .form_item("password", &args.password)
        .form_item("csrfmiddlewaretoken", csrfmiddlewaretoken)
        .form_item("next", "")
        .send(&mut socket)
        .await?;
    eprintln!("{:#?}", logged_in_res);

    let mut crawler = Crawler::new(&cookies, &args.server);
    if is_tls {
        crawler
            .visit(
                true,
                logged_in_res
                    .headers
                    .get("location")
                    .expect("Should have a location"),
            )
            .await?;
    } else {
        crawler
            .visit(
                true,
                logged_in_res
                    .headers
                    .get("location")
                    .expect("Should have a location"),
            )
            .await?;
    }

    Ok(())
}

pub async fn make_tls() -> Result<impl AsyncWriteExt + AsyncReadExt + Unpin> {
    let args = P5Args::parse();
    let host_and_port = format!("{}:{}", args.server, args.port);
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let socket = TcpStream::connect(&host_and_port).await?;
    let server_name = ServerName::try_from(args.server.clone())?;
    let socket = connector.connect(server_name, socket).await?;
    Ok(socket)
}

pub async fn make_tcp() -> Result<TcpStream> {
    let args = P5Args::parse();
    let host_and_port = format!("{}:{}", args.server, args.port);
    let socket = TcpStream::connect(&host_and_port).await?;
    Ok(socket)
    // start(socket, &args).await?;
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = P5Args::parse();
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
        start(socket, &args, true).await?;
    } else {
        let socket = TcpStream::connect(&host_and_port).await?;
        start(socket, &args, false).await?;
    }

    Ok(())
}
