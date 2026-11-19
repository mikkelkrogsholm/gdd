use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    time::Duration,
};

use serde_json::{Value, json};

#[path = "../support/server.rs"]
mod server;
pub use server::Server;

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Response {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: Value,
}

impl Response {
    pub fn read(mut stream: TcpStream) -> Self {
        let mut bytes = String::new();
        stream.read_to_string(&mut bytes).unwrap();
        let (head, body) = bytes.split_once("\r\n\r\n").unwrap();
        let mut lines = head.lines();
        let status = lines
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let headers: HashMap<String, String> = lines
            .map(|line| {
                let (name, value) = line.split_once(':').unwrap();
                (name.to_ascii_lowercase(), value.trim().to_owned())
            })
            .collect();
        let body = match headers.get("content-type").map(String::as_str) {
            Some("text/html; charset=utf-8" | "text/css; charset=utf-8") => Value::from(body),
            _ => serde_json::from_str(body).expect("JSON response"),
        };
        Self {
            status,
            headers,
            body,
        }
    }

    pub fn error(&self, status: u16, code: &str) {
        assert_eq!(self.status, status);
        assert_eq!(self.body["error"]["code"], code);
        assert!(self.body["error"]["message"].is_string());
        assert_eq!(
            self.headers["content-type"],
            "application/json; charset=utf-8"
        );
    }
}

impl Server {
    pub fn connect(&self) -> TcpStream {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, self.port));
        let stream = TcpStream::connect_timeout(&address, TIMEOUT).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TIMEOUT)).unwrap();
        stream
    }

    // Each pair is a separate header field, including repeated names.
    pub fn raw(&self, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Response {
        let mut stream = self.connect();
        write!(stream, "{method} {path} HTTP/1.1\r\nConnection: close\r\n").unwrap();
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("host"))
        {
            write!(stream, "Host: 127.0.0.1:{}\r\n", self.port).unwrap();
        }
        for (name, value) in headers {
            write!(stream, "{name}: {value}\r\n").unwrap();
        }
        stream.write_all(b"\r\n").unwrap();
        stream.write_all(body).unwrap();
        Response::read(stream)
    }

    pub fn get(&self, path: &str, headers: &[(&str, &str)]) -> Response {
        self.raw("GET", path, headers, &[])
    }

    pub fn post(&self, path: &str, body: Value, headers: &[(&str, &str)]) -> Response {
        let bytes = serde_json::to_vec(&body).unwrap();
        let length = bytes.len().to_string();
        let mut headers = headers.to_vec();
        headers.extend([
            ("Content-Type", "application/json"),
            ("Content-Length", &length),
        ]);
        self.raw("POST", path, &headers, &bytes)
    }

    pub fn login(&self, name: &str, previous: Option<&str>) -> String {
        let headers: Vec<_> = previous
            .map(|cookie| ("Cookie", cookie))
            .into_iter()
            .collect();
        let response = self.post(
            "/api/login",
            json!({"username":name, "password":format!("{name}-demo")}),
            &headers,
        );
        assert_eq!(response.status, 200);
        assert_eq!(response.body["principal"]["id"], name);
        let cookie = response.headers["set-cookie"]
            .strip_suffix("; HttpOnly; SameSite=Strict; Path=/")
            .unwrap();
        assert!(cookie.starts_with("concert_session="));
        cookie.to_owned()
    }
}
