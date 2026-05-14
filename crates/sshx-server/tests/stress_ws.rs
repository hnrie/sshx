use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use sshx::{controller::Controller, encrypt::Encrypt, runner::Runner};
use sshx_server::web::protocol::{WsClient, WsServer};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite::Message;

use crate::common::*;

pub mod common;

#[tokio::test]
async fn test_ws_broadcast_lag_resync() -> Result<()> {
    let server = TestServer::new().await;

    let mut controller = Controller::new(&server.endpoint(), "", Runner::Echo, false).await?;
    let name = controller.name().to_owned();
    let key = controller.encryption_key().to_owned();
    tokio::spawn(async move { controller.run().await });

    let endpoint = server.ws_endpoint(&name);
    let mut s1 = ClientSocket::connect(&endpoint, &key, None).await?;
    let mut s2 = ClientSocket::connect(&endpoint, &key, None).await?;
    s1.flush().await;
    s2.flush().await;

    for i in 0..5000 {
        let focus = if i % 2 == 0 { Some(sshx_core::Sid(1)) } else { None };
        s2.send(WsClient::SetFocus(focus)).await;
    }

    s1.send(WsClient::Create(0, 0)).await;
    s1.flush().await;
    assert_eq!(s1.shells.len(), 1);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
#[ignore]
async fn stress_1000_ws_clients() -> Result<()> {
    const CLIENTS: usize = 1000;

    let server = TestServer::new().await;

    let mut controller = Controller::new(&server.endpoint(), "", Runner::Echo, false).await?;
    let name = controller.name().to_owned();
    let key = controller.encryption_key().to_owned();
    tokio::spawn(async move { controller.run().await });

    let endpoint = server.ws_endpoint(&name);
    let encrypt = Encrypt::new(&key);
    let auth = WsClient::Authenticate(encrypt.zeros().into(), None);
    let mut auth_buf = Vec::new();
    ciborium::ser::into_writer(&auth, &mut auth_buf)?;
    let auth_msg = Message::Binary(auth_buf.into());

    let mut clients = Vec::with_capacity(CLIENTS);
    for _ in 0..CLIENTS {
        let (mut ws, _resp) = tokio_tungstenite::connect_async(&endpoint).await?;
        loop {
            match ws.next().await.transpose()? {
                Some(Message::Binary(msg)) => {
                    let parsed: WsServer = ciborium::de::from_reader(&*msg)?;
                    if matches!(parsed, WsServer::Hello(_, _)) {
                        break;
                    }
                }
                Some(_) => {}
                None => anyhow::bail!("server closed connection during handshake"),
            }
        }
        ws.send(auth_msg.clone()).await?;
        clients.push(ws);
    }

    sleep(Duration::from_millis(250)).await;

    for mut ws in clients {
        ws.close(None).await.ok();
    }

    Ok(())
}

