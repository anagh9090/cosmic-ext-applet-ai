use cosmic::{
    iced::futures::{Stream, StreamExt},
    iced_futures::MaybeSend,
};
use tokio::sync::mpsc;

use crate::api::{Bot, BotResponse};

#[derive(Debug, Clone)]
pub enum Event {
    Ready(mpsc::Sender<Request>),
    Response(BotResponse),
    Done,
}

#[derive(Debug, Clone)]
pub enum Request {
    Ask((String, String, Vec<String>)),
    AskWithContext((String, String, Vec<String>, Vec<(String, String)>)),
}

pub fn service() -> impl Stream<Item = Event> + MaybeSend {
    let (requests_tx, mut requests_rx) = mpsc::channel(4);
    let (responses_tx, mut responses_rx) = mpsc::channel(4);

    let service_future = async move {
        let _res = responses_tx.send(Event::Ready(requests_tx.clone())).await;
        let client = &mut None;
        while let Some(request) = requests_rx.recv().await {
            match request {
                Request::Ask((model, text, images)) => {
                    _ = client_request(model, text, images, Vec::new(), &responses_tx, client).await;
                }
                Request::AskWithContext((model, text, images, history)) => {
                    _ = client_request(model, text, images, history, &responses_tx, client).await;
                }
            }
        }
    };

    let _res = tokio::task::spawn(service_future);

    async_stream::stream! {
        while let Some(message) = responses_rx.recv().await {
            yield message;
        }
    }
}

async fn client_request<'a>(
    model: String,
    prompt: String,
    images: Vec<String>,
    history: Vec<(String, String)>,
    tx: &mpsc::Sender<Event>,
    client: &'a mut Option<Bot>,
) -> &'a mut Option<Bot> {
    *client = match Bot::new(model.to_string(), prompt, images, history, String::new()).await {
        Ok((new_client, responses)) => {
            let tx = tx.clone();
            let listener = async move {
                let mut responses = std::pin::pin!(responses);
                while let Some(Ok(response)) = responses.next().await {
                    let _res = tx.send(Event::Response(response)).await;
                }
                let _res = tx.send(Event::Done).await;
            };
            let _res = tokio::task::spawn(listener);
            Some(new_client)
        }
        Err(_) => {
            let _res = tx.send(Event::Done).await;
            None
        }
    };
    client
}
