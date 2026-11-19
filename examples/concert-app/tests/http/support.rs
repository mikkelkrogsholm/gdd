use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

use serde_json::{Value, json};

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Server {
    child: Child,
    pub port: u16,
}

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
        let headers = lines
            .map(|line| {
                let (name, value) = line.split_once(':').unwrap();
                (name.to_ascii_lowercase(), value.trim().to_owned())
            })
            .collect();
        let body = serde_json::from_str(body).expect("JSON response");
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
    pub fn start(database: &Path) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_gdd-concert"))
            .arg("--database")
            .arg(database)
            .args(["--port", "0", "--host", "localhost"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut server = Self { child, port: 0 };
        let stdout = server.child.stdout.take().unwrap();
        let (ready, address) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut line = String::new();
            BufReader::new(stdout).read_line(&mut line).unwrap();
            let _ = ready.send(line);
        });
        let line = address.recv_timeout(TIMEOUT).expect("server startup");
        reader.join().unwrap();
        server.port = line
            .trim_end()
            .strip_prefix("Local Sessions: http://127.0.0.1:")
            .expect("printed listener address")
            .parse()
            .unwrap();
        assert_ne!(server.port, 0);
        server
    }

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

    #[cfg(unix)]
    pub fn signal(&self, signal: &str) {
        assert!(
            Command::new("kill")
                .args([signal, &self.child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
    }

    #[cfg(unix)]
    pub fn wait_for_clean_exit(&mut self) {
        let deadline = std::time::Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "server exited with {status}");
                return;
            }
            assert!(std::time::Instant::now() < deadline, "server did not exit");
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
