//! Read-only load test for a running RetosEAN Rust server.
//!
//! cargo run --release --example stress -- [host:port] [seconds per step] [concurrency,...]
//! e.g. cargo run --release --example stress -- 127.0.0.1:8082 10 1,10,50,100,250,500
//!
//! Logs in once per demo role (the server allows 10 login attempts per user every
//! 10 minutes), then each worker keeps one HTTP/1.1 keep-alive connection and loops
//! over a browsing mix: dashboard, resource catalog, record lists, a record detail,
//! search, static assets and /health. No data is written apart from the 5 sessions.
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const USERS: [&str; 5] = [
    "admin.demo",
    "empresa.verificada",
    "profesor.demo",
    "estudiante.demo",
    "experto.demo",
];
const PASSWORD: &str = "RetosEAN2026";

/// Sends one request on an open connection and returns (status, body).
async fn request(
    stream: &mut TcpStream,
    host: &str,
    method: &str,
    path: &str,
    cookie: &str,
    body: &str,
) -> std::io::Result<(u16, Vec<u8>)> {
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: keep-alive\r\n");
    if !cookie.is_empty() {
        head.push_str(&format!("Cookie: retosean_session={cookie}\r\n"));
    }
    if !body.is_empty() {
        head.push_str(&format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        ));
    }
    head.push_str("\r\n");
    head.push_str(body);
    stream.write_all(head.as_bytes()).await?;
    read_response(stream).await
}

async fn read_response(stream: &mut TcpStream) -> std::io::Result<(u16, Vec<u8>)> {
    let bad = |m: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, m.to_string());
    let mut buf = Vec::with_capacity(8192);
    let mut chunk = [0u8; 16384];
    let header_end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(bad("connection closed"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_ascii_lowercase();
    let status: u16 = head
        .get(9..12)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| bad("bad status line"))?;
    let mut body = buf[header_end..].to_vec();
    if let Some(len) = head
        .lines()
        .find_map(|l| l.strip_prefix("content-length:"))
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        while body.len() < len {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                return Err(bad("short body"));
            }
            body.extend_from_slice(&chunk[..n]);
        }
        body.truncate(len);
    } else if head.contains("transfer-encoding: chunked") {
        // Read until the terminating zero-length chunk, then de-chunk.
        while !body.ends_with(b"0\r\n\r\n") {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                return Err(bad("short chunked body"));
            }
            body.extend_from_slice(&chunk[..n]);
        }
        let (mut out, mut rest) = (Vec::new(), &body[..]);
        loop {
            let line_end = rest
                .windows(2)
                .position(|w| w == b"\r\n")
                .ok_or_else(|| bad("chunk"))?;
            let size = usize::from_str_radix(
                std::str::from_utf8(&rest[..line_end]).unwrap_or("0").trim(),
                16,
            )
            .map_err(|_| bad("chunk size"))?;
            if size == 0 {
                break;
            }
            out.extend_from_slice(&rest[line_end + 2..line_end + 2 + size]);
            rest = &rest[line_end + 4 + size..];
        }
        body = out;
    }
    Ok((status, body))
}

async fn login(host: String, user: &'static str) -> Result<(String, Duration), String> {
    let mut stream = TcpStream::connect(&host).await.map_err(|e| e.to_string())?;
    let started = Instant::now();
    let body = format!(r#"{{"username":"{user}","password":"{PASSWORD}"}}"#);
    let head = format!(
        "POST /api/login HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    let mut raw = vec![0u8; 8192];
    let n = stream.read(&mut raw).await.map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&raw[..n]);
    let token = text
        .split("retosean_session=")
        .nth(1)
        .and_then(|s| s.split(';').next())
        .ok_or_else(|| {
            format!(
                "{user}: login failed: {}",
                text.lines().next().unwrap_or("")
            )
        })?;
    Ok((token.to_string(), started.elapsed()))
}

#[derive(Default)]
struct Stats {
    latencies: Vec<Duration>,
    errors: u64,
    non_2xx: u64,
    failures: std::collections::BTreeMap<String, u64>,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let host = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "127.0.0.1:8082".into());
    let seconds: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
    let levels: Vec<usize> = args
        .get(3)
        .map(|s| s.split(',').filter_map(|n| n.parse().ok()).collect())
        .unwrap_or_else(|| vec![1, 10, 50, 100, 250, 500]);

    println!("Target http://{host}, {seconds}s per step\n");
    // Logins run concurrently: PBKDF2 (1,000,000 iterations) on 4 server slots.
    let started = Instant::now();
    let results = futures_join(USERS.iter().map(|u| login(host.clone(), u)).collect()).await;
    let mut sessions = vec![];
    for (user, result) in USERS.iter().zip(results) {
        match result {
            Ok((token, took)) => {
                println!("login {user:<20} {:>7.0} ms", took.as_secs_f64() * 1000.0);
                sessions.push(token);
            }
            // A throttled account (429) is skipped; the others still carry the load.
            Err(e) => eprintln!("skipped: {e}"),
        }
    }
    if sessions.is_empty() {
        eprintln!("no session available");
        std::process::exit(1);
    }
    println!(
        "5 concurrent logins in {:.0} ms (password hashing bounds login throughput)\n",
        started.elapsed().as_secs_f64() * 1000.0
    );

    // Per session, the request mix includes the detail of a reto that role can see.
    let mut mixes: Vec<Vec<String>> = vec![];
    for cookie in &sessions {
        let mut stream = TcpStream::connect(&host).await.expect("connect");
        let (_, list) = request(
            &mut stream,
            &host,
            "GET",
            "/api/records/retos?page=1&q=",
            cookie,
            "",
        )
        .await
        .expect("list retos");
        let list: serde_json::Value = serde_json::from_slice(&list).unwrap_or_default();
        let mut paths: Vec<String> = vec![
            "/api/dashboard".into(),
            "/api/records/retos?page=1&q=".into(),
            "/api/me".into(),
            "/api/records/postulaciones?page=1&q=".into(),
            "/api/dashboard?periodo=todo".into(),
            "/api/records/notificaciones?page=1&q=".into(),
            "/api/records/retos?page=1&q=demo".into(),
            "/api/resources".into(),
            "/health".into(),
            "/app.js".into(),
        ];
        if let Some(id) = list["records"][0]["id"].as_i64() {
            paths.push(format!("/api/records/retos/{id}"));
        }
        // Keep only what this role may open (e.g. experts cannot list retos).
        let mut allowed = vec![];
        for path in paths {
            if let Ok((200..=299, _)) = request(&mut stream, &host, "GET", &path, cookie, "").await
            {
                allowed.push(path);
            }
        }
        mixes.push(allowed);
    }
    let paths = mixes.iter().max_by_key(|m| m.len()).unwrap();
    println!("Mix per worker: {}\n", paths.join(", "));
    println!(
        "{:>6} {:>9} {:>9} {:>8} {:>8} {:>8} {:>8} {:>7} {:>7}",
        "conc", "requests", "req/s", "p50 ms", "p95 ms", "p99 ms", "max ms", "errors", "non2xx"
    );

    for &concurrency in &levels {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        let mut workers = vec![];
        for w in 0..concurrency {
            let host = host.clone();
            let cookie = sessions[w % sessions.len()].clone();
            let paths = mixes[w % mixes.len()].clone();
            workers.push(tokio::spawn(async move {
                let mut stats = Stats::default();
                let mut i = w; // stagger so workers don't hit the same endpoint in lockstep
                let mut stream = None;
                while Instant::now() < deadline {
                    if stream.is_none() {
                        match TcpStream::connect(&host).await {
                            Ok(s) => stream = Some(s),
                            Err(_) => {
                                stats.errors += 1;
                                tokio::time::sleep(Duration::from_millis(50)).await;
                                continue;
                            }
                        }
                    }
                    let path = &paths[i % paths.len()];
                    i += 1;
                    let t = Instant::now();
                    match request(stream.as_mut().unwrap(), &host, "GET", path, &cookie, "").await {
                        Ok((status, _)) => {
                            stats.latencies.push(t.elapsed());
                            if !(200..300).contains(&status) {
                                stats.non_2xx += 1;
                                *stats
                                    .failures
                                    .entry(format!("{status} {path}"))
                                    .or_default() += 1;
                            }
                        }
                        Err(_) => {
                            stats.errors += 1;
                            stream = None; // reconnect
                        }
                    }
                }
                stats
            }));
        }
        let mut all = Stats::default();
        for worker in workers {
            let s = worker.await.expect("worker");
            all.latencies.extend(s.latencies);
            all.errors += s.errors;
            all.non_2xx += s.non_2xx;
            for (k, v) in s.failures {
                *all.failures.entry(k).or_default() += v;
            }
        }
        all.latencies.sort();
        let pct = |p: f64| {
            all.latencies
                .get(
                    ((all.latencies.len() as f64 * p) as usize)
                        .min(all.latencies.len().saturating_sub(1)),
                )
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0)
        };
        println!(
            "{:>6} {:>9} {:>9.0} {:>8.1} {:>8.1} {:>8.1} {:>8.1} {:>7} {:>7}",
            concurrency,
            all.latencies.len(),
            all.latencies.len() as f64 / seconds as f64,
            pct(0.50),
            pct(0.95),
            pct(0.99),
            all.latencies
                .last()
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0),
            all.errors,
            all.non_2xx
        );
        for (what, count) in &all.failures {
            println!("{:>6} non-2xx: {count} x {what}", "");
        }
    }
}

/// Awaits a set of futures concurrently, keeping their order (no extra dependency).
async fn futures_join<F: std::future::Future + Send + 'static>(futures: Vec<F>) -> Vec<F::Output>
where
    F::Output: Send + 'static,
{
    let handles: Vec<_> = futures.into_iter().map(tokio::spawn).collect();
    let mut out = vec![];
    for h in handles {
        out.push(h.await.expect("task"));
    }
    out
}
