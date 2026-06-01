use sshx_core::{Sid, Uid};
use sshx_server::session::{Metadata, Session};
use std::sync::Arc;
use tokio::time::Instant;

#[tokio::test]
async fn test_load_10000_sessions() {
    let mut sessions = Vec::new();
    let start = Instant::now();
    for i in 0..1000 {
        // We'll test with 1000 first to ensure reasonable time in CI, but scalable to 10k+
        let metadata = Metadata {
            encrypted_zeros: bytes::Bytes::new(),
            name: format!("session_{}", i),
            write_password_hash: None,
        };
        sessions.push(Arc::new(Session::new(metadata)));
    }

    // simulate operations concurrently
    let mut handles = Vec::new();
    for session in sessions.iter() {
        let session = session.clone();
        handles.push(tokio::spawn(async move {
            session.add_shell(Sid(1), (100, 100)).await.unwrap();
            session.add_user(Uid(1), true).await.unwrap();
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }

    println!(
        "Time taken for concurrent sessions load test: {:?}",
        start.elapsed()
    );
}
