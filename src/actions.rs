use super::{get_mpris_proxy, get_album_art, call_mpris_method, update_all, fetch_and_convert_to_data_url, ENCODER_PRESSED, CURRENT_AUDIO_APP_INDEX, SELECTED_SINK_INPUT};

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use openaction::*;

/// Updates the dial image based on the currently selected sink input
pub async fn update_dial_image_for_selected_sink(instance: &Instance) -> OpenActionResult<()> {
	let selected = SELECTED_SINK_INPUT.load(Ordering::Relaxed);
	log::info!("Updating dial image for selected sink input ID: {}, instance: {:?}", selected, instance.instance_id);
	if selected == 0 {
		// Master volume - set to volume icon
		let image_path = "icons/volume.png";
		log::info!("Setting master volume icon: {}", image_path);
		if let Ok(abs_path) = std::fs::canonicalize(image_path) {
			let file_url = format!("file://{}", abs_path.display());
			match fetch_and_convert_to_data_url(&file_url).await {
				Ok(data_url) => {
					log::info!("Converted to data URL (length: {})", data_url.len());
					if let Err(e) = instance.set_image(Some(data_url), None).await {
						log::error!("Failed to set master volume icon: {}", e);
					} else {
						log::info!("Successfully set master volume icon");
					}
				}
				Err(e) => {
					log::error!("Failed to convert {} to data URL: {}", image_path, e);
				}
			}
		} else {
			log::error!("Failed to find {}", image_path);
		}
		return Ok(());
	}
	
	// Specific app selected - get app info
	if let Ok(info_output) = std::process::Command::new("pactl")
		.args(&["list", "sink-inputs"])
		.output()
	{
		let info = String::from_utf8_lossy(&info_output.stdout);
		let lines = info.lines().skip_while(|line| !line.contains(&format!("Sink Input #{}", selected)));
		
		let app_name = lines.clone()
			.find(|line| line.contains("application.name"))
			.and_then(|line| line.split('"').nth(1))
			.unwrap_or("Unknown");
		
		let process_binary = lines.clone()
			.find(|line| line.contains("application.process.binary"))
			.and_then(|line| line.split('"').nth(1))
			.unwrap_or("");
		
		let app_lower = app_name.to_lowercase();
		let process_lower = process_binary.to_lowercase();
		
		// Check if it's a media player or browser that might have metadata
		let is_media_app = app_lower.contains("firefox") || app_lower.contains("chrome") 
			|| app_lower.contains("brave") || app_lower.contains("spotify")
			|| app_lower.contains("vlc") || process_lower.contains("mpv");
		
		let mut image_set = false;
		
		if is_media_app {
			log::info!("Attempting to fetch album art for media application: {}", app_name);
			// Try to get MPRIS metadata for album art
			let proxy_result = get_mpris_proxy().await;
			let get_property = async |property: &str| match &proxy_result {
				Ok(proxy) => proxy.get_property(property).await.ok(),
				Err(_) => None,
			};
			
			if let Some(album_art) = get_album_art(get_property("Metadata").await.as_ref()).await {
				if let Err(e) = instance.set_image(Some(album_art), None).await {
					log::warn!("Failed to set album art: {}", e);
				} else {
					log::info!("Successfully set album art");
					image_set = true;
				}
			}
		}
		
		if !image_set {
			// Try to find icon by process name
			let possible_names = vec![
				process_binary,
				&app_lower,
				&process_lower,
			];
			
			log::info!("Looking for icon matching: {:?}", possible_names);
			
			for name in possible_names {
				if name.is_empty() { continue; }
				
				for ext in &["svg", "png", "jpg", "jpeg"] {
					let icon_path = format!("icons/{}.{}", name, ext);
					// Check if file exists in plugin directory
					if std::path::Path::new(&icon_path).exists() {
						log::info!("Found icon: {}", icon_path);
						if let Ok(abs_path) = std::fs::canonicalize(&icon_path) {
							let file_url = format!("file://{}", abs_path.display());
							match fetch_and_convert_to_data_url(&file_url).await {
								Ok(data_url) => {
									log::info!("Converted to data URL (length: {})", data_url.len());
									if let Err(e) = instance.set_image(Some(data_url), None).await {
										log::warn!("Failed to set icon: {}", e);
									} else {
										log::info!("Successfully set icon");
									}
								}
								Err(e) => {
									log::warn!("Failed to convert {} to data URL: {}", icon_path, e);
								}
							}
						}
						image_set = true;
						break;
					}
				}
				if image_set { break; }
			}
			
			if !image_set {
				log::warn!("No icon found for app: {} [{}], using unknown.png", app_name, process_binary);
				// Use unknown.png as fallback
				let fallback_path = "icons/unknown.png";
				log::info!("Setting fallback unknown icon: {}", fallback_path);
				if let Ok(abs_path) = std::fs::canonicalize(fallback_path) {
					let file_url = format!("file://{}", abs_path.display());
					match fetch_and_convert_to_data_url(&file_url).await {
						Ok(data_url) => {
							log::info!("Converted to data URL (length: {})", data_url.len());
							if let Err(e) = instance.set_image(Some(data_url), None).await {
								log::error!("Failed to set unknown icon: {}", e);
							} else {
								log::info!("Successfully set unknown icon");
							}
						}
						Err(e) => {
							log::error!("Failed to convert {} to data URL: {}", fallback_path, e);
						}
					}
				} else {
					log::error!("Failed to find {}", fallback_path);
				}
			}
		}
	} else {
		log::error!("Failed to get sink input info for ID {}", selected);
	}
	
	Ok(())
}

pub struct VolumeDialAction;
#[async_trait]
impl Action for VolumeDialAction {
	const UUID: ActionUuid = "PlayMix.volumedialaction";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		_: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		if ENCODER_PRESSED.load(Ordering::Relaxed) {
			// When pressed, cycle through audio-producing programs (with master volume as first option)
			if let Ok(output) = std::process::Command::new("pactl")
				.args(&["list", "sink-inputs", "short"])
				.output()
			{
				let stdout = String::from_utf8_lossy(&output.stdout);
				let sink_inputs: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
				
				// Total items = 1 (master) + number of sink inputs
				let total_items = sink_inputs.len() + 1;
				let current_index = CURRENT_AUDIO_APP_INDEX.load(Ordering::Relaxed);
				
				// Calculate new index based on rotation direction
				let new_index = if ticks > 0 {
					(current_index + 1) % total_items
				} else {
					if current_index == 0 {
						total_items - 1
					} else {
						current_index - 1
					}
				};
				
				CURRENT_AUDIO_APP_INDEX.store(new_index, Ordering::Relaxed);
				
				if new_index == 0 {
					// Master volume selected
					SELECTED_SINK_INPUT.store(0, Ordering::Relaxed);
					log::info!("Switched to: Master Volume (1 of {})", total_items);
				} else {
					// Specific app selected (index - 1 because master is at 0)
					let sink_index = new_index - 1;
					if let Some(sink_input_line) = sink_inputs.get(sink_index) {
						if let Some(sink_input_id_str) = sink_input_line.split_whitespace().next() {
							if let Ok(sink_input_id) = sink_input_id_str.parse::<usize>() {
								SELECTED_SINK_INPUT.store(sink_input_id, Ordering::Relaxed);
								
								// Get application name for logging
								if let Ok(info_output) = std::process::Command::new("pactl")
									.args(&["list", "sink-inputs"])
									.output()
								{
									let info = String::from_utf8_lossy(&info_output.stdout);
									let lines = info.lines().skip_while(|line| !line.contains(&format!("Sink Input #{}", sink_input_id)));
									
									let app_name = lines.clone()
										.find(|line| line.contains("application.name"))
										.and_then(|line| line.split('"').nth(1))
										.unwrap_or("Unknown");
									
									let process_binary = lines.clone()
										.find(|line| line.contains("application.process.binary"))
										.and_then(|line| line.split('"').nth(1))
										.unwrap_or("");
									
									log::info!("Switched to audio app: {} [{}] (ID: {}, {} of {})", 
										app_name, process_binary, sink_input_id, new_index + 1, total_items);
								}
							}
						}
					}
				}
				
				// Update the image for the selected sink
				update_dial_image_for_selected_sink(instance).await?;
			} else {
				log::error!("Failed to list audio applications");
			}
			return Ok(());
		}
		
		// Volume control when not pressed - adjust selected source
		let selected = SELECTED_SINK_INPUT.load(Ordering::Relaxed);
		
		if selected == 0 {
			// Master volume
			let volume_change = if ticks > 0 {
				format!("{}%+", ticks.abs() * 5)
			} else {
				format!("{}%-", ticks.abs() * 5)
			};
			
			if let Err(error) = std::process::Command::new("wpctl")
				.args(&["set-volume", "@DEFAULT_AUDIO_SINK@", &volume_change, "--limit", "1.0"])
				.output()
			{
				log::error!("Failed to change master volume: {}", error);
			} else {
				log::info!("Changed master volume by {}", volume_change);
			}
		} else {
			// Specific app volume - pactl uses +/- prefix format
			let volume_change = if ticks > 0 {
				format!("+{}%", ticks.abs() * 5)
			} else {
				format!("-{}%", ticks.abs() * 5)
			};
			
			log::info!("Changing app {} volume by {}", selected, volume_change);
			
			if let Err(error) = std::process::Command::new("pactl")
				.args(&["set-sink-input-volume", &selected.to_string(), &volume_change])
				.output()
			{
				log::error!("Failed to change app volume: {}", error);
			} else {
				log::info!("Changed app {} volume by {}", selected, volume_change);
			}
		}
		
		Ok(())
	}

	async fn dial_down(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		ENCODER_PRESSED.store(true, Ordering::Relaxed);
		log::info!("Volume dial pressed");
		Ok(())
	}

	async fn dial_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		ENCODER_PRESSED.store(false, Ordering::Relaxed);
		log::info!("Volume dial released");
		Ok(())
	}
}

pub struct DialTestAction;
#[async_trait]
impl Action for DialTestAction {
	const UUID: ActionUuid = "PlayMix.dialtestaction";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn dial_rotate(
		&self,
		instance: &Instance,
		_: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		log::info!("Dial rotated on instance {}: ticks = {}", instance.instance_id, ticks);
		log::info!("Dial pressed state: {}", ENCODER_PRESSED.load(Ordering::Relaxed));
		Ok(())

	}

	async fn dial_down(&self, instance: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		ENCODER_PRESSED.store(true, Ordering::Relaxed);
		log::info!("Dial button pressed on instance {}", instance.instance_id);
		Ok(())
	}

	async fn dial_up(&self, instance: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		ENCODER_PRESSED.store(false, Ordering::Relaxed);
		log::info!("Dial button released on instance {}", instance.instance_id);
		Ok(())
	}
}

pub struct PlayPauseAction;
#[async_trait]
impl Action for PlayPauseAction {
	const UUID: ActionUuid = "PlayMix.playpause";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		log::info!("PlayPause key_up triggered");
		if let Err(error) = call_mpris_method("PlayPause").await {
			log::error!("Failed to make PlayPause MPRIS call: {}", error);
		}
		Ok(())
	}
}

pub struct StopAction;
#[async_trait]
impl Action for StopAction {
	const UUID: ActionUuid = "PlayMix.stop";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		if let Err(error) = call_mpris_method("Stop").await {
			log::error!("Failed to make Stop MPRIS call: {}", error);
		}
		Ok(())
	}
}

pub struct PreviousAction;
#[async_trait]
impl Action for PreviousAction {
	const UUID: ActionUuid = "PlayMix.previous";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		if let Err(error) = call_mpris_method("Previous").await {
			log::error!("Failed to make Previous MPRIS call: {}", error);
		}
		Ok(())
	}
}

pub struct NextAction;
#[async_trait]
impl Action for NextAction {
	const UUID: ActionUuid = "PlayMix.next";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		if let Err(error) = call_mpris_method("Next").await {
			log::error!("Failed to make Next MPRIS call: {}", error);
		}
		Ok(())
	}
}