//! Standalone checker compiled by the host process tests.

use std::{
    env, fs,
    io::{self, Read, Write},
    path::Path,
    process::{self, Command},
    thread,
    time::{Duration, Instant},
};

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref().unwrap() {
        mode @ ("echo" | "echo-args") => {
            let code = args.next().unwrap().parse().unwrap();
            let args = args.collect::<Vec<_>>();
            if mode == "echo" {
                assert_eq!(io::stdin().read(&mut [0]).unwrap(), 0);
            }
            io::stdout().write_all(args.join("\n").as_bytes()).unwrap();
            eprint!(
                "{}:{}",
                env::var("CHECK_VALUE").unwrap(),
                fs::read_to_string("marker").unwrap()
            );
            process::exit(code);
        }
        "streams" => thread::scope(|scope| {
            scope.spawn(|| io::stdout().write_all(&vec![0xff; 512 * 1024]).unwrap());
            io::stderr().write_all(&vec![0xfe; 512 * 1024]).unwrap();
        }),
        "file-check" => {
            fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open("invocations")
                .unwrap()
                .write_all(b"run\n")
                .unwrap();
            for action in args {
                match action.as_str() {
                    "output" => {
                        io::stdout().write_all(b"out\xff\r\nend\r").unwrap();
                        io::stdout().flush().unwrap();
                        io::stderr().write_all(b"err\xfe\rbar\r\n").unwrap();
                        io::stderr().flush().unwrap();
                    }
                    "abort" => process::abort(),
                    "change" => fs::write("selected", b"changed").unwrap(),
                    "remove" => fs::remove_file("selected").unwrap(),
                    "wait" => {
                        let start = Instant::now();
                        while !Path::new("release").exists() {
                            assert!(start.elapsed() < Duration::from_secs(12));
                            thread::sleep(Duration::from_millis(10));
                        }
                    }
                    other => panic!("unexpected file action {other}"),
                }
            }
        }
        "timeout" => {
            let lock = fs::File::create("checker.lock").unwrap();
            lock.lock().unwrap();
            fs::write("checker.pid", process::id().to_string()).unwrap();
            io::stdout().write_all(b"partial stdout\xff").unwrap();
            io::stdout().flush().unwrap();
            io::stderr().write_all(b"partial stderr\xfe").unwrap();
            let mut holder = Command::new(env::current_exe().unwrap())
                .arg("hold")
                .spawn()
                .unwrap();
            holder.wait().unwrap();
        }
        "hold" => {
            fs::write("holding", b"").unwrap();
            let start = Instant::now();
            while !Path::new("release").exists() && start.elapsed() < Duration::from_secs(12) {
                thread::sleep(Duration::from_millis(10));
            }
            fs::write("released", b"").unwrap();
        }
        other => panic!("unexpected checker mode {other}"),
    }
}
