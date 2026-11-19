use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
};

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Server {
    child: Child,
    pub port: u16,
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
