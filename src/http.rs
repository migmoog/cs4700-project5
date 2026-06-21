use std::collections::BTreeMap;

mod parse;
use anyhow::{Result, anyhow};
use enum_stringify::EnumStringify;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::http::parse::http_response;

type Headers = BTreeMap<String, String>;

#[derive(Debug, EnumStringify)]
#[enum_stringify(case = "upper")]
pub enum Method {
    Get,
    Post,
}

const CRLF: &'static str = "\r\n";
const HTTP_END: &'static str = "\r\n\r\n";

#[derive(Debug)]
pub struct RequestBuilder {
    method: Method,
    path: String,
    headers: Headers,
}

impl RequestBuilder {
    pub fn new(method: Method, path: String) -> Self {
        Self {
            method,
            path,
            headers: Headers::new(),
        }
    }

    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub async fn send<T>(&self, socket: &mut T) -> Result<Response>
    where
        T: AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let s = self.to_string();
        socket.write_all(s.as_bytes()).await?;

        let mut response_buffer = Vec::with_capacity(2048);
        socket.read_buf(&mut response_buffer).await?;
        Response::try_from(
            str::from_utf8(&response_buffer).expect("Should be a valid ascii sequence"),
        )
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
        out
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Response {
    pub code: u32,
    pub message: String,
    pub headers: Headers,
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

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

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
                headers: BTreeMap::from([
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
