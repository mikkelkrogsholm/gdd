use ntex::http::{
    HeaderMap, Request,
    header::{self, HeaderValue},
};
use serde_json::{Map, Value};

use super::error::Error;

const MAX_BODY: usize = 16 * 1024;
const INVALID: Error = Error::InvalidInput("Send a valid JSON object of at most 16 KiB.");

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a HeaderValue> {
    let mut values = headers.get_all(name);
    let first = values.next()?;
    values.next().is_none().then_some(first)
}

pub fn check_local(headers: &HeaderMap, hosts: &[String; 2]) -> Result<(), Error> {
    let host = single(headers, "host")
        .and_then(|v| v.to_str().ok())
        .filter(|v| hosts.iter().any(|host| host == v))
        .ok_or(Error::Forbidden("Use the local app address."))?;
    if headers.contains_key(header::ORIGIN)
        && single(headers, "origin").and_then(|v| v.as_bytes().strip_prefix(b"http://"))
            != Some(host.as_bytes())
    {
        return Err(Error::Forbidden("Use the app from its local origin."));
    }
    Ok(())
}

pub fn token(headers: &HeaderMap) -> Option<&str> {
    let cookies = single(headers, "cookie")?.to_str().ok()?;
    let mut token = None;
    for pair in cookies.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, value) = pair.split_once('=')?;
        if name.trim() == "concert_session" {
            let value = value.trim();
            token = Some(
                value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .unwrap_or(value),
            );
        }
    }
    token
}

pub fn expect_principal(headers: &HeaderMap, id: Option<&str>) -> Result<(), Error> {
    let mut expectations = headers.get_all("x-concert-principal");
    let first = expectations.next();
    if expectations.next().is_some() {
        return Err(Error::InvalidInput(
            "Send at most one identity expectation.",
        ));
    }
    if let (Some(expected), Some(id)) = (first, id)
        && expected.as_bytes() != id.as_bytes()
    {
        return Err(Error::SessionChanged);
    }
    Ok(())
}

pub struct JsonObject(Map<String, Value>);

impl JsonObject {
    pub fn string(&self, key: &str) -> Option<&str> {
        self.0.get(key)?.as_str()
    }

    pub fn length(headers: &HeaderMap) -> Result<usize, Error> {
        let length = single(headers, "content-length");
        let media_type = single(headers, "content-type");
        let (Some(length), Some(media_type), false) = (
            length,
            media_type,
            headers.contains_key(header::TRANSFER_ENCODING),
        ) else {
            return Err(Error::invalid_framing());
        };
        if !media_type
            .to_str()
            .ok()
            .and_then(|s| s.split(';').next())
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("application/json"))
        {
            return Err(Error::InvalidInput("Send an application/json object."));
        }
        length
            .to_str()
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|&n| n <= MAX_BODY)
            .ok_or(INVALID)
    }

    pub async fn read(request: &mut Request, length: usize) -> Result<Self, Error> {
        let mut bytes = Vec::with_capacity(length);
        while let Some(chunk) = request.payload().recv().await {
            let chunk = chunk.map_err(|_| INVALID)?;
            if chunk.len() > length - bytes.len() {
                return Err(INVALID);
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() != length {
            return Err(INVALID);
        }
        serde_json::from_slice(&bytes)
            .map(Self)
            .map_err(|_| INVALID)
    }
}
