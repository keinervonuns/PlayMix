use super::{call_mpris_method, update_all, ENCODER_PRESSED};

use std::collections::HashMap;
use std::sync::atomic::Ordering;

use openaction::*;

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
		_: &Instance,
		_: &Self::Settings,
		ticks: i16,
		_pressed: bool,
	) -> OpenActionResult<()> {
		if ENCODER_PRESSED.load(Ordering::Relaxed) {
			log::info!("Volume dial rotated while pressed; ignoring rotation");
			return Ok(());
		}
		let volume_change = if ticks > 0 {
			format!("{}%+", ticks.abs() * 5)
		} else {
			format!("{}%-", ticks.abs() * 5)
		};
		
		if let Err(error) = std::process::Command::new("wpctl")
			.args(&["set-volume", "@DEFAULT_AUDIO_SINK@", &volume_change, "--limit", "1.0"])
			.output()
		{
			log::error!("Failed to change volume: {}", error);
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