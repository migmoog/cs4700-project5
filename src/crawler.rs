use anyhow::{Result, anyhow};
use scraper::Selector;
use std::collections::HashSet;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    http::{Method, RequestBuilder},
    make_tcp, make_tls,
};

pub struct Crawler {
    visited_urls: HashSet<String>,
    cookies: String,
    server: String,
}

impl Crawler {
    pub fn new(cookies: &str, server: &str) -> Self {
        Self {
            visited_urls: HashSet::from(["/accounts/login/".to_string()]),
            cookies: cookies.to_string(),
            server: server.to_string(),
        }
    }

    pub async fn visit(&mut self, is_tls: bool, path: &str) -> Result<()> {
        if !path.starts_with("/") {
            eprintln!("skipping {path}");
            return Ok(());
        } else {
            self.visited_urls.insert(path.to_string());
        }

        let builder = RequestBuilder::new(Method::Get, path.to_string())
            .header("Host", &self.server)
            .header("Cookie", &self.cookies);
        let response = if is_tls {
            let mut socket = make_tls().await?;
            let response = builder.send(&mut socket).await?;
            socket.shutdown().await?;
            response
        } else {
            let mut socket = make_tcp().await?;
            let response = builder.send(&mut socket).await?;
            socket.shutdown().await?;
            response
        };

        if !response
            .headers
            .get("content-type")
            .is_some_and(|v| v.contains("text/html"))
        {
            eprintln!("{:#?}", response);
            return Ok(());
        }
        let doc = match response.html_body() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{:#?}", response);
                return Err(e);
            },
        };
        let flag_selector = Selector::parse("h3.secret_flag")
            .map_err(|e| anyhow!("Couldn't make flag selector {e}"))?;
        for secret_flag in doc.select(&flag_selector) {
            for text in secret_flag.text() {
                println!("{}", text);
            }
        }

        let link_selector =
            Selector::parse("a").map_err(|e| anyhow!("Couldn't make link selector {e}"))?;
        for link in doc.select(&link_selector).map(|e| e.attr("href")) {
            if let Some(url) = link {
                if self.visited_urls.contains(url) {
                    continue;
                }
                Box::pin(self.visit(is_tls, url)).await?;
            }
        }

        Ok(())
    }
}
