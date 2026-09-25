//! Programmable HTTP speaker mock. Exercises the real client and SOAP parser.
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    task::{JoinHandle, JoinSet},
};

#[derive(Clone)]
pub struct SoapReply {
    pub status: u16,
    pub body: String,
    pub delay: Duration,
}
impl SoapReply {
    pub fn value(field: &str, value: &str) -> Self {
        Self {
            status: 200,
            body: format!("<Response><{field}>{value}</{field}></Response>"),
            delay: Duration::ZERO,
        }
    }
    pub fn fault(code: u16) -> Self {
        Self {
            status: 500,
            body: format!(
                "<s:Envelope><s:Body><s:Fault><detail><UPnPError><errorCode>{code}</errorCode></UPnPError></detail></s:Fault></s:Body></s:Envelope>"
            ),
            delay: Duration::ZERO,
        }
    }
}

pub struct MockSoapServer {
    pub address: std::net::SocketAddr,
    replies: Arc<Mutex<HashMap<String, SoapReply>>>,
    requests: Arc<Mutex<Vec<String>>>,
    task: JoinHandle<()>,
}
impl MockSoapServer {
    pub async fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let replies = Arc::new(Mutex::new(HashMap::<String, SoapReply>::new()));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let server_replies = replies.clone();
        let server_requests = requests.clone();
        let task = tokio::spawn(async move {
            let mut clients = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = listener.accept() => {
                        let Ok((mut stream, _)) = accepted else { break };
                        let replies = server_replies.clone();
                        let requests = server_requests.clone();
                        clients.spawn(async move {
                            let mut request = Vec::new();
                            loop {
                                let mut chunk = [0; 4096];
                                let Ok(size) = stream.read(&mut chunk).await else { return };
                                if size == 0 { return; }
                                request.extend_from_slice(&chunk[..size]);
                                if request.len() > 65536 { return; }
                                let text = String::from_utf8_lossy(&request);
                                if let Some((headers, body)) = text.split_once("\r\n\r\n") {
                                    let length = headers.lines().find_map(|line| {
                                        let (name, value) = line.split_once(':')?;
                                        name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse::<usize>().ok()).flatten()
                                    }).unwrap_or(0);
                                    if body.len() >= length { break; }
                                }
                            }
                            let request = String::from_utf8_lossy(&request).into_owned();
                            let action = request.lines().find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                if !name.eq_ignore_ascii_case("soapaction") { return None; }
                                value.split_once('#').map(|(_, action)| action.trim_end_matches('"').to_owned())
                            }).unwrap_or_default();
                            let key = super::xml_value(&request, "EQType").map_or_else(|| action.clone(), |eq| format!("{action}:{eq}"));
                            requests.lock().await.push(request);
                            let reply = replies.lock().await.get(&key).cloned().unwrap_or_else(|| SoapReply::fault(401));
                            tokio::time::sleep(reply.delay).await;
                            let response = format!("HTTP/1.1 {} Mock\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", reply.status, reply.body.len(), reply.body);
                            let _ = stream.write_all(response.as_bytes()).await;
                        });
                    }
                    _ = clients.join_next(), if !clients.is_empty() => {}
                }
            }
        });
        Ok(Self {
            address,
            replies,
            requests,
            task,
        })
    }
    pub async fn reply(&self, action: &str, reply: SoapReply) {
        self.replies.lock().await.insert(action.to_owned(), reply);
    }
    pub async fn requests(&self) -> Vec<String> {
        self.requests.lock().await.clone()
    }
}
impl Drop for MockSoapServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}
