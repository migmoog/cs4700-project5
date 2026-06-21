use nom::{
    IResult, Parser,
    bytes::{
        complete::{take_until, take_while1},
        streaming::tag,
    },
    character::complete::{digit1, line_ending, not_line_ending, space0, space1},
    combinator::{map_res, rest},
    multi::many0,
};

use crate::http::Response;

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

pub fn http_response(input: &str) -> IResult<&str, Response> {
    let (input, (code, message)) = initial_response_line(input)?;
    let (input, headers) = many0(header_line).parse(input)?;
    let (input, _) = line_ending(input)?;
    let (input, body) = rest(input)?;

    let body = (!body.is_empty()).then_some(body.to_string());

    Ok((
        input,
        Response {
            code,
            message,
            headers: headers.into_iter().collect(),
            body,
        },
    ))
}
