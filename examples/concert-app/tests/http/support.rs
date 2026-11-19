use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpStream},
    time::Duration,
};

use serde_json::{Value, json};

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
        if let Some(length) = headers.get("content-length") {
            assert_eq!(body.len(), length.parse::<usize>().unwrap(), "body length");
        }
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

pub trait HttpClient {
    fn address(&self) -> SocketAddr;

    fn connect(&self) -> TcpStream {
        let address = self.address();
        let stream = TcpStream::connect_timeout(&address, TIMEOUT).unwrap();
        stream.set_read_timeout(Some(TIMEOUT)).unwrap();
        stream.set_write_timeout(Some(TIMEOUT)).unwrap();
        stream
    }

    // Each pair is a separate header field, including repeated names.
    fn raw(&self, method: &str, path: &str, headers: &[(&str, &str)], body: &[u8]) -> Response {
        let mut request = Vec::new();
        write!(request, "{method} {path} HTTP/1.1\r\nConnection: close\r\n").unwrap();
        if !headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("host"))
        {
            write!(request, "Host: {}\r\n", self.address()).unwrap();
        }
        for (name, value) in headers {
            write!(request, "{name}: {value}\r\n").unwrap();
        }
        request.extend_from_slice(b"\r\n");
        request.extend_from_slice(body);

        let mut stream = self.connect();
        if let Err(error) = stream.write_all(&request) {
            // A peer can send its final response before consuming the request body.
            assert!(
                matches!(
                    error.kind(),
                    ErrorKind::BrokenPipe | ErrorKind::ConnectionReset
                ),
                "writing HTTP request: {error}"
            );
        }
        Response::read(stream)
    }

    fn get(&self, path: &str, headers: &[(&str, &str)]) -> Response {
        self.raw("GET", path, headers, &[])
    }

    fn post(&self, path: &str, body: Value, headers: &[(&str, &str)]) -> Response {
        let bytes = serde_json::to_vec(&body).unwrap();
        let length = bytes.len().to_string();
        let mut headers = headers.to_vec();
        headers.extend([
            ("Content-Type", "application/json"),
            ("Content-Length", &length),
        ]);
        self.raw("POST", path, &headers, &bytes)
    }

    fn login(&self, name: &str, previous: Option<&str>) -> String {
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
