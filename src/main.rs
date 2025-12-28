mod actions;

use actions::*;

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use futures_util::StreamExt;
use openaction::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use zbus::fdo::DBusProxy;
use zbus::{Connection, MatchRule, MessageStream, Proxy};
use zbus::message::Type as MessageType;
use zvariant::Value;

pub static ENCODER_PRESSED: AtomicBool = AtomicBool::new(false);
pub static CURRENT_AUDIO_APP_INDEX: AtomicUsize = AtomicUsize::new(0);
pub static SELECTED_SINK_INPUT: AtomicUsize = AtomicUsize::new(0); // 0 = master volume

async fn fetch_and_convert_to_data_url(url: &str) -> Result<String> {
	let bytes = if url.starts_with("data:") {
		return Ok(url.to_owned());
	} else if url.starts_with("file:") {
		let path = url.trim_start_matches("file://");
		std::fs::read(path)?
	} else {
		let response = reqwest::get(url).await?;
		response.bytes().await?.to_vec()
	};

	let mime_type = infer::get(&bytes)
		.map(|info| info.mime_type())
		.unwrap_or("application/octet-stream");
	let base64_data = general_purpose::STANDARD.encode(&bytes);
	Ok(format!("data:{};base64,{}", mime_type, base64_data))
}

async fn find_active_player(conn: &Connection) -> Result<String> {
	let proxy = Proxy::new(
		conn,
		"org.freedesktop.DBus",
		"/org/freedesktop/DBus",
		"org.freedesktop.DBus",
	)
	.await?;

	let names: Vec<String> = proxy.call("ListNames", &()).await?;
	names
		.into_iter()
		.find(|name| name.starts_with("org.mpris.MediaPlayer2."))
		.ok_or_else(|| anyhow::anyhow!("No MPRIS players found"))
}

async fn get_mpris_proxy() -> Result<Proxy<'static>> {
	let conn = Connection::session().await?;
	let player_name = find_active_player(&conn).await?;

	let proxy = Proxy::new(
		&conn,
		player_name,
		"/org/mpris/MediaPlayer2",
		"org.mpris.MediaPlayer2.Player",
	)
	.await?;

	Ok(proxy)
}

async fn call_mpris_method(method: &str) -> Result<()> {
	let proxy = get_mpris_proxy().await?;
	proxy.call_method(method, &()).await?;
	Ok(())
}

async fn get_album_art(metadata: Option<&Value<'_>>) -> Option<String> {
	let dict = metadata?.downcast_ref::<zvariant::Dict>().ok()?;
	let url: String = dict.get(&Value::from("mpris:artUrl")).ok()??;
	fetch_and_convert_to_data_url(&url).await.ok()
}

async fn update_play_pause(instance: &Instance, image: Option<String>) -> OpenActionResult<()> {
	instance.set_image(image, None).await
}

async fn update_all() {
	let proxy_result = get_mpris_proxy().await;
	let get_property = async |property: &str| match &proxy_result {
		Ok(proxy) => proxy.get_property(property).await.ok(),
		Err(_) => None,
	};
	for instance in visible_instances(PlayPauseAction::UUID).await {
		if let Err(error) = update_play_pause(
			&instance,
			get_album_art(get_property("Metadata").await.as_ref()).await,
		)
		.await
		{
			log::error!("Failed to update PlayPause: {}", error);
		}
	}
}

async fn watch_album_art() {
	let connection = match Connection::session().await {
		Ok(conn) => conn,
		Err(error) => {
			log::error!("Failed to connect to DBus session: {}", error);
			return;
		}
	};

	loop {
		update_all().await;

		let player_name = match find_active_player(&connection).await {
			Ok(name) => name,
			Err(_) => {
				tokio::time::sleep(std::time::Duration::from_secs(1)).await;
				continue;
			}
		};

		let dbus_proxy = match DBusProxy::new(&connection).await {
			Ok(proxy) => proxy,
			Err(error) => {
				log::error!("Failed to create DBus proxy: {}", error);
				return;
			}
		};

		let signal_rule = match MatchRule::builder()
			.msg_type(MessageType::Signal)
			.interface("org.freedesktop.DBus.Properties")
			.and_then(|b| b.member("PropertiesChanged"))
			.and_then(|b| b.path("/org/mpris/MediaPlayer2"))
			.and_then(|b| b.sender(player_name.as_str()))
			.map(|b| b.build())
		{
			Ok(rule) => rule,
			Err(error) => {
				log::error!("Failed to build match rule: {}", error);
				continue;
			}
		};

		if let Err(error) = dbus_proxy.add_match_rule(signal_rule).await {
			log::error!("Failed to add match rule: {}", error);
			continue;
		}

		let name_owner_rule = MatchRule::builder()
			.msg_type(MessageType::Signal)
			.interface("org.freedesktop.DBus")
			.and_then(|b| b.member("NameOwnerChanged"))
			.map(|b| b.build());

		if let Ok(rule) = name_owner_rule {
			let _ = dbus_proxy.add_match_rule(rule).await;
		}

		let mut stream = MessageStream::from(&connection);

		while let Some(msg_result) = stream.next().await {
			let msg = match msg_result {
				Ok(m) => m,
				Err(error) => {
					log::error!("Error receiving message: {}", error);
					continue;
				}
			};

			let header = msg.header();

			let member = header.member().map(|m| m.to_string());
			if member.as_deref() == Some("NameOwnerChanged") {
				let body = msg.body();
				if let Ok((name, _old_owner, new_owner)) = body.deserialize::<(String, String, String)>()
					&& name == player_name
					&& new_owner.is_empty()
				{
					break;
				}
				continue;
			} else if member.as_deref() != Some("PropertiesChanged") {
				continue;
			}

			let body = msg.body();
			let (interface, changed_properties, _): (String, HashMap<String, Value>, Vec<String>) = match body.deserialize() {
				Ok(b) => b,
				Err(error) => {
					log::error!("Error reading message body: {}", error);
					continue;
				}
			};

			if interface != "org.mpris.MediaPlayer2.Player" {
				continue;
			}

			if let Some(playback_status_value) = changed_properties.get("PlaybackStatus") {
				if let Ok(status_str) = playback_status_value.downcast_ref::<zvariant::Str>() {
					if status_str.as_str() == "Stopped" {
						update_all().await;
						continue;
					}
				}
			}

			let album_art_url = get_album_art(changed_properties.get("Metadata")).await;

			for instance in visible_instances(PlayPauseAction::UUID).await {
				if let Err(error) = update_play_pause(&instance, album_art_url.clone()).await {
					log::error!("Failed to update PlayPause: {}", error);
				}
			}
			for instance in visible_instances(VolumeDialAction::UUID).await {
				update_dial_image_for_selected_sink(&instance).await.unwrap_or_else(|e| {
					log::error!("Failed to update dial image: {}", e);
				});
			}
		}
	}
}

#[tokio::main]
async fn main() -> OpenActionResult<()> {
	simplelog::TermLogger::init(
		simplelog::LevelFilter::Info,
		simplelog::Config::default(),
		simplelog::TerminalMode::Stdout,
		simplelog::ColorChoice::Never,
	)
	.unwrap();

	// log::info!("Args: {:?}", std::env::args().collect::<Vec<_>>());

	register_action(PlayPauseAction {}).await;
	register_action(StopAction {}).await;
	register_action(PreviousAction {}).await;
	register_action(NextAction {}).await;
	register_action(VolumeDialAction {}).await;
	register_action(DialTestAction {}).await;

	tokio::spawn(watch_album_art());

	run(std::env::args().collect()).await
}
