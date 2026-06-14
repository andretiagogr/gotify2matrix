use anyhow::{anyhow, Result};
use futures_util::{Stream, StreamExt};
use reqwest::{header, Client as HttpClient};
use serde::Deserialize;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Message as WsMessage};
use tracing::warn;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct Application {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Message {
    pub id: i64,
    pub appid: i64,
    pub title: Option<String>,
    pub message: String,
    pub priority: i32,
}

#[derive(Debug, Deserialize)]
pub struct Paging {
    pub next: Option<String>,
    pub since: i64,
}

#[derive(Debug, Deserialize)]
pub struct PagedMessages {
    pub messages: Vec<Message>,
    pub paging: Paging,
}

pub struct GotifyClient {
    http: HttpClient,
    base_url: Url,
    token: String,
}

impl GotifyClient {
    pub fn new(base_url: &Url, token: &str) -> Result<Self> {
        // Ensure the base URL ends in `/` so `Url::join` preserves any sub-path
        // (e.g. a reverse-proxied `https://host/gotify/`) instead of replacing
        // the last path segment.
        let mut base_url = base_url.clone();
        if !base_url.path().ends_with('/') {
            let path = format!("{}/", base_url.path());
            base_url.set_path(&path);
        }

        let mut headers = header::HeaderMap::new();
        headers.insert("X-Gotify-Key", header::HeaderValue::from_str(token)?);

        let http = HttpClient::builder()
            .default_headers(headers)
            .build()?;

        Ok(GotifyClient {
            http,
            base_url,
            token: token.to_string(),
        })
    }

    pub async fn get_applications(&self) -> Result<Vec<Application>> {
        let url = self.base_url.join("application")?;
        let apps = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(apps)
    }

    pub async fn get_messages(&self, since: Option<i64>) -> Result<PagedMessages> {
        let mut url = self.base_url.join("message")?;
        if let Some(since) = since {
            url.query_pairs_mut().append_pair("since", &since.to_string());
        }
        let paged = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(paged)
    }

    pub async fn delete_message(&self, id: i64) -> Result<()> {
        let url = self.base_url.join(&format!("message/{}", id))?;
        self.http.delete(url).send().await?.error_for_status()?;
        Ok(())
    }

    pub async fn stream_messages(&self) -> Result<impl Stream<Item = Result<Message>> + Send> {
        let mut ws_url = self.base_url.join("stream")?;
        ws_url
            .set_scheme(match ws_url.scheme() {
                "https" => "wss",
                _ => "ws",
            })
            .map_err(|_| anyhow!("Failed to set WebSocket scheme"))?;

        let mut request = ws_url
            .as_str()
            .into_client_request()
            .map_err(|e| anyhow!("Failed to build WebSocket request: {e}"))?;
        request.headers_mut().insert(
            "X-Gotify-Key",
            header::HeaderValue::from_str(&self.token)?,
        );

        let (ws, _) = tokio_tungstenite::connect_async(request)
            .await
            .map_err(|e| anyhow!("WebSocket connection failed: {e}"))?;

        let stream = futures_util::stream::unfold(ws, |mut ws| async move {
            loop {
                match ws.next().await {
                    // Skip (don't tear down the stream over) a single frame we
                    // can't parse; only a transport error or a clean close ends it.
                    Some(Ok(WsMessage::Text(text))) => match serde_json::from_str::<Message>(&*text)
                    {
                        Ok(msg) => return Some((Ok(msg), ws)),
                        Err(e) => {
                            warn!("Skipping unparseable gotify stream frame: {e}");
                            continue;
                        }
                    },
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => return Some((Err(anyhow::Error::from(e)), ws)),
                    None => return None,
                }
            }
        });

        Ok(Box::pin(stream))
    }
}
