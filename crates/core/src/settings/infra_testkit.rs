//! Test stand in for an S3 endpoint (SPEC-0117): a local listener that answers every
//! request with a chosen status, counts the requests by path and can hold a
//! connection open without answering. Test only.
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct Shared {
    status: AtomicU16,
    hang: AtomicBool,
    hits: Mutex<Vec<String>>,
    holds: Mutex<BTreeSet<String>>,
}

pub(crate) struct S3Stand {
    pub url: String,
    shared: Arc<Shared>,
}

impl S3Stand {
    /// A stand in answering `status` to every request.
    pub(crate) async fn start(status: u16) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let shared = Arc::new(Shared {
            status: AtomicU16::new(status),
            hang: AtomicBool::new(false),
            hits: Mutex::new(Vec::new()),
            holds: Mutex::new(BTreeSet::new()),
        });
        let s = shared.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else { return };
                let s = s.clone();
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 4096];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        match sock.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&chunk[..n]),
                        }
                    }
                    let text = String::from_utf8_lossy(&buf).to_string();
                    let line = text.lines().next().unwrap_or_default().to_string();
                    let path = line.split_whitespace().nth(1).unwrap_or_default().to_string();
                    s.hits.lock().unwrap().push(path.clone());
                    if s.hang.load(Ordering::SeqCst) {
                        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                        return;
                    }
                    let held = s.holds.lock().unwrap().iter().any(|b| path.starts_with(b));
                    let status = if held { 200 } else { s.status.load(Ordering::SeqCst) };
                    let reply = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    let _ = sock.write_all(reply.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        Self { url, shared }
    }

    pub(crate) fn set_status(&self, status: u16) {
        self.shared.status.store(status, Ordering::SeqCst);
    }

    /// Never answer: the connection stays open.
    pub(crate) fn hang(&self) {
        self.shared.hang.store(true, Ordering::SeqCst);
    }

    /// Answer present (200) for every object of `bucket`.
    pub(crate) fn holds_bucket(&self, bucket: &str) {
        self.shared.holds.lock().unwrap().insert(format!("/{bucket}/"));
    }

    /// The request paths seen so far, in order.
    pub(crate) fn hits(&self) -> Vec<String> {
        self.shared.hits.lock().unwrap().clone()
    }
}
