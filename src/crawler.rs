use anyhow::{Result, anyhow};
use scraper::{Html, Selector};
use std::{
    collections::{HashSet, VecDeque},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

use crate::{
    http::{Method, RequestBuilder},
    make_tcp, make_tls,
};

pub struct Crawler {
    visited_urls: HashSet<String>,
    trips_queue: VecDeque<String>,
    cookies: String,
    server: String,
}

impl Crawler {
    pub fn new(cookies: &str, server: &str) -> Self {
        Self {
            visited_urls: HashSet::from(["/accounts/login/".to_string()]),
            trips_queue: VecDeque::new(),
            cookies: cookies.to_string(),
            server: server.to_string(),
        }
    }

    pub async fn scan(&mut self, is_tls: bool, initial_path: &str) -> Result<()> {
        self.trips_queue.push_back(initial_path.to_string());
        while let Some(path) = self.trips_queue.pop_front() {
            self.visit(is_tls, &path).await?;
        }

        Ok(())
    }

    pub async fn visit(&mut self, is_tls: bool, path: &str) -> Result<()> {
        if !path.starts_with("/") {
            eprintln!("skipping {path}");
            return Ok(());
        } else if path.contains("accounts") {
            // eprintln!("Session related link \"{}\". Ignoring", path);
            return Ok(());
        } else if self.visited_urls.contains(path) {
            eprintln!("Already visited {path}");
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

        let out = match response.code {
            200 => {
                let doc = response.html_body()?;
                eprintln!("200: collecting links");
                self.collect_links(doc)
            }

            // Found, redirect
            302 => {
                let Some(location) = response.headers.get("location") else {
                    return Err(anyhow!("Got a 302 without a location"));
                };
                eprintln!("302: redirected to {}", location);
                Box::pin(self.visit(is_tls, location)).await
            }

            503 => {
                eprintln!("503: Waiting like a good lad");
                tokio::time::sleep(Duration::from_secs(2)).await;
                self.collect_links(response.html_body()?)
            }

            c => panic!("Got an unhandled code: {c}"),
        };
        eprintln!("Finished visiting {path}");
        out
    }

    /// Collects the links in an html doc and sends more accompanying requests
    pub fn collect_links(&mut self, doc: Html) -> Result<()> {
        let flag_selector = Selector::parse(".secret_flag")
            .map_err(|e| anyhow!("Couldn't make flag selector {e}"))?;
        for secret_flag in doc.select(&flag_selector) {
            for text in secret_flag.text() {
                println!("{}", text.replace("FLAG: ", ""));
            }
        }

        let link_selector =
            Selector::parse("a").map_err(|e| anyhow!("Couldn't make link selector {e}"))?;
        for link in doc.select(&link_selector).map(|e| e.attr("href")) {
            if let Some(url) = link {
                if self.visited_urls.contains(url) {
                    continue;
                }
                self.trips_queue.push_back(url.to_string());
            }
        }

        Ok(())
    }
}
