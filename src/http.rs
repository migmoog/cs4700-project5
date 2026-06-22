use std::collections::HashMap;

mod parse;
use anyhow::{Result, anyhow};
use enum_stringify::EnumStringify;
use indexmap::IndexMap;
use scraper::Html;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::http::parse::http_response;

type Headers = IndexMap<String, String>;

#[derive(Debug, EnumStringify)]
#[enum_stringify(case = "upper")]
pub enum Method {
    Get,
    Post,
}

const CRLF: &'static str = "\r\n";

#[derive(Debug)]
pub struct RequestBuilder {
    method: Method,
    path: String,
    headers: Headers,
    form_items: String,
}

impl RequestBuilder {
    pub fn new(method: Method, path: String) -> Self {
        Self {
            method,
            path,
            headers: Headers::new(),
            form_items: String::new(),
        }
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn form_item(mut self, name: &str, value: &str) -> Self {
        self.headers
            .entry("Content-Type".to_string())
            .or_insert("application/x-www-form-urlencoded".to_string());

        let current_content_len = self.form_items.len();
        let formatted_item = format!(
            "{}{}={}",
            if current_content_len > 0 { "&" } else { "" },
            name,
            value
        );
        self.form_items.push_str(&formatted_item);
        self.headers
            .entry("Content-Length".to_string())
            .and_modify(|s| {
                *s = self.form_items.len().to_string();
            })
            .or_insert(self.form_items.len().to_string());
        self
    }

    pub async fn send<T>(&self, socket: &mut T) -> Result<Response>
    where
        T: AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let s = self.to_string();
        // eprintln!("=========Sending http request=========\n{}", s);
        socket.write_all(s.as_bytes()).await?;

        let mut response_buffer = Vec::with_capacity(2048);

        // eprintln!("-------Reading response from socket------");
        let bytes_read = socket.read_buf(&mut response_buffer).await?;
        if bytes_read == 0 {
            return Err(anyhow!("Socket disconnected"));
        }

        let response_str = str::from_utf8(&response_buffer[..bytes_read])
            .expect("Should be a valid ascii sequence");
        // eprintln!("{}", response_str);

        let mut r = Response::try_from(response_str)?;

        if r.should_keep_alive() {
            eprintln!("SHOULD KEEP THIS ALIVE");
        }

        if r.code == 503 {
            return Ok(r);
        }

        if let Some(len) = r.content_length()
            && len > 0
        {
            let mut content = Vec::with_capacity(len);
            let bytes_read = socket.read_buf(&mut content).await?;
            if bytes_read == 0 {
                eprintln!("Disconnected, need {len} bytes");
            } else {
                if let Ok(result) = String::from_utf8(content) {
                    // eprintln!("{}", result);
                    r.body = Some(result);
                }
            }
        }

        Ok(r)
    }
}

impl ToString for RequestBuilder {
    fn to_string(&self) -> String {
        let mut out = format!("{} {} HTTP/1.1{CRLF}", self.method.to_string(), self.path);
        // add headers
        for (k, v) in self.headers.iter() {
            out.push_str(k);
            out.push_str(": ");
            out.push_str(v);
            out.push_str(CRLF);
        }
        out.push_str(CRLF);

        if !self.form_items.is_empty() {
            out.push_str(&self.form_items);
        }

        out
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Response {
    pub code: u32,
    pub message: String,
    pub headers: Headers,
    pub set_cookies: HashMap<String, String>,
    pub body: Option<String>,
}

impl<'a> TryFrom<&'a str> for Response {
    type Error = anyhow::Error;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        let (_, r) =
            http_response(value).map_err(|e| anyhow!("Couldn't parse http response: {e}"))?;

        Ok(r)
    }
}

impl Response {
    pub fn content_length(&self) -> Option<usize> {
        let v = self.headers.get("content-length")?;
        v.parse().ok()
    }

    pub fn html_body(&self) -> Result<Html> {
        if let Some(body) = self.body.as_ref() {
            let d = Html::parse_document(body);
            Ok(d)
        } else {
            Err(anyhow!("HTTP response has no body"))
        }
    }

    pub fn cookies(&self) -> String {
        self.set_cookies
            .iter()
            .map(|(ck, cv)| format!("{ck}={cv}"))
            .fold(String::new(), |mut s, pair| {
                if !s.is_empty() {
                    s.push_str("; ");
                }
                s.push_str(&pair);
                s
            })
    }

    pub fn should_keep_alive(&self) -> bool {
        self.headers
            .get("connection")
            .is_some_and(|e| e.eq_ignore_ascii_case("keep-alive"))
    }

    pub fn is_chunked(&self) -> bool {
        // Transfer-Encoding can be a list like "gzip, chunked"
        self.headers.get("transfer-encoding").is_some_and(|v| {
            v.split(',')
                .any(|t| t.trim().eq_ignore_ascii_case("chunked"))
        })
    }
}

#[cfg(test)]
mod test {

    use std::collections::HashMap;

    use indexmap::IndexMap;

    use crate::http::Response;

    #[test]
    fn parse_response() {
        let input = r#"HTTP/1.1 200 OK
Date: Fri, 31 Dec 1999 23:59:59 GMT
Content-Type: text/plain
Transfer-Encoding: chunked

1a; ignore-stuff-here
abcdefghijklmnopqrstuvwxyz
10
1234567890abcdef
0
some-footer: some-value
another-footer: another-value"#;

        assert_eq!(
            Response::try_from(input).unwrap(),
            Response {
                code: 200,
                message: "OK".to_string(),
                set_cookies: HashMap::new(),
                headers: IndexMap::from([
                    (
                        "Date".to_string(),
                        "Fri, 31 Dec 1999 23:59:59 GMT".to_string()
                    ),
                    ("Content-Type".to_string(), "text/plain".to_string()),
                    ("Transfer-Encoding".to_string(), "chunked".to_string())
                ]),
                body: Some(
                    r#"1a; ignore-stuff-here
abcdefghijklmnopqrstuvwxyz
10
1234567890abcdef
0
some-footer: some-value
another-footer: another-value"#
                        .to_string()
                )
            }
        );
    }
}
