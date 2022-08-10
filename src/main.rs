use matrix_sdk::{
    config::SyncSettings,
    room::{Joined, Room},
    ruma::{
        events::room::message::{
            MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
            TextMessageEventContent,
        },
        user_id,
    },
    Client,
};
use oops::Oops;
use reqwest;
use serde::Deserialize;
use serde_json;
use std::io::Result;
use urlencoding::encode;

#[derive(Debug)]
enum Action<'a> {
    NativeQuery(&'a str),
    DocQuery(&'a str),
}

#[derive(Deserialize)]
struct NativeResponse(String);

#[derive(Deserialize)]
struct Annotation {
    name: String,
    value: String,
}

#[derive(Deserialize)]
struct Parameter {
    doc: Option<String>,
    name: String,

    #[serde(rename = "type")]
    type_: String,
}

#[derive(Deserialize)]
struct DocResponse {
    annotations: Vec<Annotation>,
    commit: String,
    kind: String,
    linenumber: String,
    parameters: Vec<Parameter>,
}

fn parse(input: &str) -> Result<Action> {
    let mut input = input.split(' ');
    let command = input.next().oops("Single word messages are ignored")?;
    let arg = input.next().oops("Second word is absent")?;

    let res = match command {
        "!j" | "!jass" => Ok(Action::NativeQuery(arg)),
        "!d" | "!doc" => Ok(Action::DocQuery(arg)),
        _ => None.oops("First word is not a trigger"),
    };

    println!("{:?}", res);
    res
}

async fn handle_native_query(room: Joined, content: &str) -> Result<()> {
    println!("querying");

    let json_str = reqwest::get(format!(
        "https://lep.duckdns.org/app/jassbot/search/api/{}",
        encode(content)
    ))
    .await
    .oops("Request failed")?;

    let query = serde_json::from_str::<Vec<NativeResponse>>(
        &json_str.text().await.oops("Failed to get body")?,
    )
    .oops("Failed to deserialize response")?;

    let content = query
        .into_iter()
        .take(3)
        .map(|nr| nr.0)
        .collect::<Vec<_>>()
        .join("\n");

    println!("sending");

    room.send(RoomMessageEventContent::text_plain(content), None)
        .await
        .unwrap();

    println!("message sent");
    Ok(())
}

async fn handle_doc_query(room: Joined, content: &str) -> Result<()> {
    println!("querying");

    let json_str = reqwest::get(format!(
        "https://lep.duckdns.org/app/jassbot/doc/api/{}",
        encode(content)
    ))
    .await
    .oops("Request failed")?;

    let query =
        serde_json::from_str::<DocResponse>(&json_str.text().await.oops("Failed to get body")?)
            .oops("Failed to deserialize response")?;

    let annotations = query
        .annotations
        .into_iter()
        .filter(|an| an.name != "return-type")
        .filter(|an| an.name != "source-code")
        .filter(|an| an.name != "source-file")
        .map(|an| format!("{}: {}", an.name, an.value))
        .collect::<Vec<_>>();

    let parameters = query
        .parameters
        .into_iter()
        .filter(|pa| pa.doc.is_some())
        .map(|pa| format!("{} parameter: {}", pa.name, pa.doc.unwrap()))
        .collect::<Vec<_>>();

    if annotations.len() + parameters.len() == 0 {
        return Ok(());
    }

    let content = annotations.join("\n") + &parameters.join("\n");

    println!("sending");

    room.send(RoomMessageEventContent::text_plain(content), None)
        .await
        .unwrap();

    println!("message sent");
    Ok(())
}

async fn on_room_message(event: OriginalSyncRoomMessageEvent, room: Room) -> Result<()> {
    if let Room::Joined(room) = room {
        let msg_body = match event.content.msgtype {
            MessageType::Text(TextMessageEventContent { body, .. }) => body,
            _ => return Ok(()),
        };
        println!("{}", msg_body);

        match parse(&msg_body) {
            Ok(Action::NativeQuery(query)) => handle_native_query(room, query).await?,
            Ok(Action::DocQuery(query)) => handle_doc_query(room, query).await?,
            Err(_) => return Ok(()),
        };
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let alice = user_id!("@jassbot:matrix.org");
    let client = Client::builder()
        .user_id(alice)
        .build()
        .await
        .oops("Failed to build client")?;
    let password = std::env::var("PASSWORD").oops("Missing PASSWORD env variable")?;

    client
        .login(alice, &password, None, None)
        .await
        .oops("Failed to login to matrix.org")?;

    // Don't respond to old messages.
    client.sync_once(SyncSettings::default()).await.unwrap();

    client.register_event_handler(on_room_message).await;

    client
        .sync(SyncSettings::default().token(client.sync_token().await.unwrap()))
        .await;

    Ok(())
}
