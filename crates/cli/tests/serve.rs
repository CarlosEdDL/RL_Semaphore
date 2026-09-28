//! End-to-end test of `rl-semaphore serve` (R9.8): runs the built binary, reads the bound
//! address from the `listening` event on stderr, checks `/healthz`, and confirms an
//! `RL_SEMAPHORE_*` override reached the server before killing the process.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
const EXAMPLE: &str = "configs/single-intersection.toml";
const TIMEOUT: Duration = Duration::from_secs(10);

/// A JSON event line, with its `fields` sub-object flattened out for convenience.
fn find_event<'a>(lines: &'a [serde_json::Value], message: &str) -> Option<&'a serde_json::Value> {
    lines
        .iter()
        .find(|line| line["fields"]["message"] == message)
}

struct Running {
    child: Child,
    lines: Vec<serde_json::Value>,
}

impl Running {
    /// Starts `rl-semaphore serve` with `--log-format json`, and reads stderr lines (each
    /// parsed as JSON) until `message` appears or the timeout elapses.
    fn start_until(extra_args: &[&str], envs: &[(&str, &str)], message: &str) -> Self {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_rl-semaphore"));
        cmd.current_dir(ROOT)
            .args(["--log-format", "json", "serve", "--config", EXAMPLE])
            .args(extra_args)
            .env_remove("RUST_LOG")
            .env_remove("RL_SEMAPHORE_LOG_FORMAT")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in envs {
            cmd.env(key, value);
        }
        let mut child = cmd.spawn().expect("failed to spawn rl-semaphore");
        let stderr = child.stderr.take().expect("piped stderr");
        let mut reader = BufReader::new(stderr);
        let mut lines = Vec::new();

        let deadline = Instant::now() + TIMEOUT;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).expect("failed to read stderr");
            let value: serde_json::Value = serde_json::from_str(line.trim())
                .unwrap_or_else(|e| panic!("not JSON ({e}): {line}"));
            let is_target = value["fields"]["message"] == message;
            lines.push(value);
            if is_target {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for the {message:?} event; saw: {lines:?}"
            );
        }
        Self { child, lines }
    }

    fn addr(&self) -> String {
        find_event(&self.lines, "listening")
            .and_then(|line| line["fields"]["addr"].as_str())
            .expect("listening event with an addr field")
            .to_owned()
    }

    fn event(&self, message: &str) -> &serde_json::Value {
        find_event(&self.lines, message).unwrap_or_else(|| panic!("no {message:?} event"))
    }

    /// Sends SIGTERM (via the `kill` utility, so no unsafe signal call is needed here) and
    /// waits for a clean exit (R8.4).
    fn stop(mut self) {
        let status = Command::new("kill")
            .args(["-TERM", &self.child.id().to_string()])
            .status()
            .expect("failed to run kill");
        assert!(status.success(), "kill -TERM failed: {status:?}");
        let status = self.child.wait().expect("failed to wait on child");
        assert!(status.success(), "expected a clean exit, got {status:?}");
    }
}

/// A minimal blocking HTTP/1.1 GET, closing the connection after one response.
fn http_get(addr: &str, path: &str) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).expect("connect failed");
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("set_read_timeout");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )
    .expect("write failed");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).expect("read failed");
    let text = String::from_utf8_lossy(&buf).into_owned();
    let mut parts = text.splitn(2, "\r\n\r\n");
    let head = parts.next().unwrap_or_default();
    let body = parts.next().unwrap_or_default().to_owned();
    let status = head
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok())
        .expect("status line");
    (status, body)
}

#[test]
fn serve_binds_answers_healthz_and_shuts_down_cleanly() {
    let running = Running::start_until(
        &["--bind", "127.0.0.1:0", "--speed", "1000"],
        &[("RL_SEMAPHORE_SEED", "99")],
        "listening",
    );

    // The env override reached the server (R8.1).
    assert_eq!(running.event("serve started")["fields"]["seed"], 99);

    let (status, body) = http_get(&running.addr(), "/healthz");
    assert_eq!(status, 200);
    let health: serde_json::Value = serde_json::from_str(&body).expect("valid json body");
    assert_eq!(health["status"], "ok");
    assert!(health["protocol_version"].is_u64());

    running.stop();
}
