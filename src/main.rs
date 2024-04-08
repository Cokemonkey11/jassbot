pub mod jassdoc;

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
use std::io::{ErrorKind, Result};

use crate::jassdoc::{
    jassdoc_doc_response_of, jassdoc_native_response_of, jassdoc_user_doc_uri_of,
};

#[derive(Debug)]
enum Action<'a> {
    NativeQuery(&'a str),
    DocQuery(&'a str),
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

    res
}

async fn handle_native_query(room: Joined, content: &str) -> Result<()> {
    println!("querying");

    let query = jassdoc_native_response_of(content).await?;

    let content = query
        .results
        .into_iter()
        .take(3)
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

    let query = jassdoc_doc_response_of(content).await?;

    let uri = jassdoc_user_doc_uri_of(content);

    let annotations = query
        .annotations
        .into_iter()
        .filter(|an| an.name != "return-type")
        .filter(|an| an.name != "source-code")
        .filter(|an| an.name != "source-file")
        .map(|an| format!("{}: {}", an.name, an.value.trim()))
        .collect::<Vec<_>>();

    let parameters = query
        .parameters
        .into_iter()
        .filter(|pa| pa.doc.is_some())
        .map(|pa| format!("* {} parameter: {}", pa.name, pa.doc.unwrap().trim()))
        .collect::<Vec<_>>();

    if annotations.len() + parameters.len() == 0 {
        return Ok(());
    }

    let content = uri + "\n \n" + &annotations.join("\n \n") + "\n \n" + &parameters.join("\n");

    println!("sending");

    room.send(RoomMessageEventContent::text_markdown(content), None)
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

        let res = match parse(&msg_body) {
            Ok(Action::NativeQuery(query)) => handle_native_query(room.clone(), query).await,
            Ok(Action::DocQuery(query)) => handle_doc_query(room.clone(), query).await,
            Err(_) => return Ok(()),
        };

        if let Some(err) = res.err() {
            if err.kind() == ErrorKind::Other && err.to_string().contains("Not Found") {
                room.send(
                    RoomMessageEventContent::text_markdown("No results found"),
                    None,
                )
                .await
                .unwrap();
            }
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let user = user_id!("@jassbot:matrix.org");
    let client = Client::builder()
        .user_id(user)
        .build()
        .await
        .oops("Failed to build client")?;
    let password = std::env::var("PASSWORD").oops("Missing PASSWORD env variable")?;

    client
        .login(user, &password, None, None)
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
