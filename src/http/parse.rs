use std::collections::HashMap;

use nom::{
    IResult, Parser,
    bytes::{
        complete::{take_until, take_while1},
        streaming::tag,
        take_till,
    },
    character::complete::{digit1, line_ending, not_line_ending, space0, space1},
    combinator::{map_res, rest},
    multi::many0,
    sequence::separated_pair,
};

use crate::http::{Headers, Response};

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
    let (input, _) = line_ending(input)?;
    Ok((input, (key.to_string(), value.trim_end().to_string())))
}

fn set_cookie_header(input: &str) -> IResult<&str, (String, String)> {
    separated_pair(
        nom::bytes::complete::take_until("="),
        tag("="),
        nom::bytes::complete::take_till(|c| c == ';'),
    )
    .parse(input)
    .map(|(input, (ck, cv))| {
        (
            input,
            (ck.trim_end().to_string(), cv.trim_end().to_string()),
        )
    })
}

pub fn http_response(input: &str) -> IResult<&str, Response> {
    let (input, (code, message)) = initial_response_line(input)?;
    let (input, raw_headers) = many0(header_line).parse(input)?;
    let (input, _) = line_ending(input)?;
    let (input, body) = rest(input)?;

    let body = (!body.is_empty()).then_some(body.to_string());

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
