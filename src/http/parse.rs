use std::collections::HashMap;

use nom::{
    IResult, Parser,
    bytes::{
        complete::{take_till, take_until, take_while1},
        streaming::tag,
        take,
    },
    character::complete::{digit1, line_ending, not_line_ending, space0, space1},
    combinator::{map_res, opt, rest},
    multi::{many0, many1},
    sequence::separated_pair,
};

use crate::http::{Headers, Response, header_has};

fn initial_response_line(input: &str) -> IResult<&str, (u32, String)> {
    let (input, _) = tag("HTTP/1.1")(input)?;
    let (input, _) = space1(input)?;
    let mut code_parser = map_res(digit1, |o: &str| o.parse::<u32>());
    let (input, code) = code_parser.parse(input)?;
    let (input, _) = space1(input)?;
    let (input, message) = not_line_ending(input)?;
    let (input, _) = line_ending(input)?;

    Ok((input, (code, message.to_string())))
}

fn header_line(input: &str) -> IResult<&str, (String, String)> {
    let (input, key) = take_while1(|c: char| c != ':' && c != '\r' && c != '\n')(input)?;
    let (input, _) = tag(":")(input)?;
    let (input, _) = space0(input)?;
    let (input, value) = not_line_ending(input)?;
    let (input, _) = opt(line_ending).parse(input)?;
    Ok((input, (key.to_string(), value.trim_end().to_string())))
}

fn set_cookie_header(input: &str) -> IResult<&str, (String, String)> {
    separated_pair(take_until("="), tag("="), take_till(|c| c == ';'))
        .parse(input)
        .map(|(input, (ck, cv))| {
            (
                input,
                (ck.trim_end().to_string(), cv.trim_end().to_string()),
            )
        })
}

// parses a chunked encoding
fn chunk(input: &str) -> IResult<&str, String> {
    let (input, chunk_size) = map_res(
        take_till(|c| c == ';' || c == '\r' || c == '\n'),
        |s: &str| {
            eprintln!("C0) chunk size {s:?}");
            usize::from_str_radix(s, 16)
        },
    )
    .parse(input)?;
    if chunk_size == 0 {
        return Ok((input, ("".to_string())));
    }

    let (input, (_needless_past_semicolon, _crlf)) = (not_line_ending, line_ending).parse(input)?;
    eprintln!("C1) chunk content: {input}");

    let (input, chunk) = take(chunk_size).parse(input)?;
    eprintln!("C2) remaining chunk: {input}");
    let (input, _) = line_ending(input)?;
    Ok((input, chunk.to_string()))
}

pub fn http_response(input: &str) -> IResult<&str, Response> {
    let (input, (code, message)) = initial_response_line(input)?;
    let (input, raw_headers) = many0(header_line).parse(input)?;
    let (mut input, _) = line_ending(input)?;

    let mut headers = Headers::new();
    let mut set_cookies = HashMap::new();
    for (key, value) in raw_headers {
        if key.eq_ignore_ascii_case("set-cookie") {
            if let Ok((_, (ck, cv))) = set_cookie_header(&value) {
                set_cookies.insert(ck, cv);
            }
        } else {
            headers.insert(key, value);
        }
    }

    let mut body = String::new();
    if header_has(&headers, "Transfer-Encoding", "chunked") {
        input = rest(input)?.1;
        let (remaining, chunks) = many1(chunk).parse(input)?;
        for chunk in chunks {
            body.push_str(&chunk);
        }
        input = remaining;
        // parse footers if there are any
        if !input.is_empty() {
            let (input, _) = line_ending(input)?;
            eprintln!("Searching for footers {input}");
            let (_input, footers) = many0(header_line).parse(input)?;
            for (fk, fv) in footers {
                headers.insert(fk, fv);
            }
        }
    } else {
        body.push_str(&input);
    }

    let body = (!body.is_empty()).then_some(body.to_string());

    Ok((
        input,
        Response {
            code,
            message,
            headers,
            set_cookies,
            body,
        },
    ))
}
