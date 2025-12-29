mod actions;

use actions::*;

use anyhow::Result;
use base64::{Engine as _, engine::general_purpose};
use futures_util::StreamExt;
use openaction::*;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Mutex, atomic::{AtomicBool}};
use zbus::fdo::DBusProxy;
use zbus::{Connection, MatchRule, MessageStream, Proxy};
use zbus::message::Type as MessageType;
use zvariant::Value;

pub static ENCODER_PRESSED: AtomicBool = AtomicBool::new(false);

// Per-instance state: (current_audio_app_index, selected_sink_input)
pub static DIAL_STATES: Lazy<Mutex<HashMap<String, (usize, usize)>>> = Lazy::new(|| Mutex::new(HashMap::new()));

// Remember the last active MPRIS player
pub static LAST_ACTIVE_PLAYER: Lazy<Mutex<Option<String>>> = Lazy::new(|| Mutex::new(None));

pub async fn fetch_and_convert_to_data_url(url: &str) -> Result<String> {
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
	let mpris_players: Vec<String> = names
		.into_iter()
		.filter(|name| name.starts_with("org.mpris.MediaPlayer2.") && name != "org.mpris.MediaPlayer2.playerctld")
		.collect();
	
	// Try to find a player that is actively playing
	for player_name in &mpris_players {
		if let Ok(player_proxy) = Proxy::new(
			conn,
			player_name.as_str(),
			"/org/mpris/MediaPlayer2",
			"org.mpris.MediaPlayer2.Player",
		).await {
			if let Ok(status) = player_proxy.get_property::<String>("PlaybackStatus").await {
				if status == "Playing" {
					log::info!("Found active player: {} (Playing)", player_name);
					// Remember this as the last active player
					*LAST_ACTIVE_PLAYER.lock().unwrap() = Some(player_name.clone());
					return Ok(player_name.clone());
				}
			}
		}
	}
	
	// If no player is actively playing, try to use the last active one
	if let Some(last_player) = LAST_ACTIVE_PLAYER.lock().unwrap().clone() {
		if mpris_players.contains(&last_player) {
			log::info!("No active player, using last active: {}", last_player);
			return Ok(last_player);
		}
	}
	
	// Fallback to first player if none are actively playing and no last player remembered
	let first_player = mpris_players
		.into_iter()
		.next()
		.ok_or_else(|| anyhow::anyhow!("No MPRIS players found"))?;
	
	log::info!("No active or remembered player, using first available: {}", first_player);
	Ok(first_player)
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

/// Find all MPRIS players for a given process name (e.g., "brave", "firefox")
async fn find_mpris_players_for_app(app_name: &str) -> Vec<String> {
	let conn = match Connection::session().await {
		Ok(c) => c,
		Err(_) => return vec![],
	};
	
	let proxy = match Proxy::new(
		&conn,
		"org.freedesktop.DBus",
		"/org/freedesktop/DBus",
		"org.freedesktop.DBus",
	)
	.await {
		Ok(p) => p,
		Err(_) => return vec![],
	};

	let names: Vec<String> = match proxy.call("ListNames", &()).await {
		Ok(n) => n,
		Err(_) => return vec![],
	};
	
	let search_pattern = format!("org.mpris.MediaPlayer2.{}", app_name.to_lowercase());
	names
		.into_iter()
		.filter(|name| name.starts_with(&search_pattern))
		.collect()
}

/// Try to get album art from a specific MPRIS player instance
async fn get_album_art_from_player(player_name: &str) -> Option<String> {
	let conn = Connection::session().await.ok()?;
	let proxy = Proxy::new(
		&conn,
		player_name,
		"/org/mpris/MediaPlayer2",
		"org.mpris.MediaPlayer2.Player",
	)
	.await.ok()?;
	
	let metadata = proxy.get_property("Metadata").await.ok()?;
	get_album_art(Some(&metadata)).await
}

/// Get album art for a specific sink input by matching it with the corresponding MPRIS instance
/// When there are multiple tabs/sources from the same app, this tries to match them by index
pub async fn get_album_art_for_sink_input(sink_input_id: usize, process_binary: &str) -> Option<String> {
	// Get full sink input list once
	let info_output = std::process::Command::new("pactl")
		.args(&["list", "sink-inputs"])
		.output()
		.ok()?;
	
	let info = String::from_utf8_lossy(&info_output.stdout);
	
	// Parse all sink inputs and filter by process binary
	let mut sink_inputs: Vec<usize> = Vec::new();
	let mut current_id: Option<usize> = None;
	let mut in_matching_app = false;
	
	for line in info.lines() {
		if line.starts_with("Sink Input #") {
			// Save previous entry if it matches
			if in_matching_app {
				if let Some(id) = current_id {
					sink_inputs.push(id);
				}
			}
			// Reset for new entry
			current_id = line.trim_start_matches("Sink Input #").parse().ok();
			in_matching_app = false;
		} else if line.contains("application.process.binary") {
			if let Some(binary) = line.split('"').nth(1) {
				if binary == process_binary {
					in_matching_app = true;
				}
			}
		}
	}
	
	// Don't forget the last entry
	if in_matching_app {
		if let Some(id) = current_id {
			sink_inputs.push(id);
		}
	}
	
	sink_inputs.sort(); // Sort to get consistent ordering
	
	// Find the index of our sink input
	let sink_index = sink_inputs.iter().position(|&id| id == sink_input_id)?;
	
	log::info!("Sink input {} is at index {} among {} total sink inputs for {} (IDs: {:?})", 
		sink_input_id, sink_index, sink_inputs.len(), process_binary, sink_inputs);
	
	// Get all MPRIS instances for this app, sorted
	let mut mpris_players = find_mpris_players_for_app(process_binary).await;
	mpris_players.sort(); // Sort to get consistent ordering
	
	log::info!("Found {} MPRIS players for {}: {:?}", mpris_players.len(), process_binary, mpris_players);
	
	// If there are more sink inputs than MPRIS players, we can't reliably match them
	// This happens with Chromium browsers where multiple tabs share one MPRIS interface
	if sink_inputs.len() > mpris_players.len() {
		log::warn!("More sink inputs ({}) than MPRIS players ({}) - cannot reliably match tabs to metadata. Skipping album art.", 
			sink_inputs.len(), mpris_players.len());
		return None;
	}
	
	// Try to match by index
	if sink_index < mpris_players.len() {
		let matched_player = &mpris_players[sink_index];
		log::info!("Matched sink input {} (index {}) to MPRIS player: {}", sink_input_id, sink_index, matched_player);
		
		if let Some(album_art) = get_album_art_from_player(matched_player).await {
			log::info!("Successfully got album art from matched player {}", matched_player);
			return Some(album_art);
		} else {
			log::warn!("Failed to get album art from matched player {}", matched_player);
		}
	} else {
		log::warn!("Index {} out of range for {} MPRIS players", sink_index, mpris_players.len());
	}
	
	// Fallback: try all instances
	log::info!("Index matching failed or no album art, trying all {} MPRIS instances as fallback", mpris_players.len());
	for player in mpris_players {
		if let Some(album_art) = get_album_art_from_player(&player).await {
			log::info!("Got fallback album art from {}", player);
			return Some(album_art);
		}
	}
	
	log::warn!("No album art found for sink input {} ({})", sink_input_id, process_binary);
	None
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
				log::info!("Updating dial image for instance {:?}", instance.instance_id);
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
