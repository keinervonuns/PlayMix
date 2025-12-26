use super::{call_mpris_method, cycle_repeat_mode, seek, toggle_shuffle, update_all};

use std::collections::HashMap;

use openaction::*;

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

pub struct RepeatAction;
#[async_trait]
impl Action for RepeatAction {
	const UUID: ActionUuid = "PlayMix.repeat";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		if let Err(error) = cycle_repeat_mode().await {
			log::error!("Failed to make Repeat MPRIS call: {}", error);
		}
		Ok(())
	}
}

pub struct ShuffleAction;
#[async_trait]
impl Action for ShuffleAction {
	const UUID: ActionUuid = "PlayMix.shuffle";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		if let Err(error) = toggle_shuffle().await {
			log::error!("Failed to make Shuffle MPRIS call: {}", error);
		}
		Ok(())
	}
}

pub struct SeekBackwardsAction;
#[async_trait]
impl Action for SeekBackwardsAction {
	const UUID: ActionUuid = "PlayMix.seekbackwards";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		if let Err(error) = seek(-10_000_000).await {
			log::error!("Failed to make Seek MPRIS call: {}", error);
		}
		Ok(())
	}
}

pub struct SeekForwardsAction;
#[async_trait]
impl Action for SeekForwardsAction {
	const UUID: ActionUuid = "PlayMix.seekforwards";
	type Settings = HashMap<String, String>;

	async fn will_appear(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		update_all().await;
		Ok(())
	}

	async fn key_up(&self, _: &Instance, _: &Self::Settings) -> OpenActionResult<()> {
		if let Err(error) = seek(10_000_000).await {
			log::error!("Failed to make Seek MPRIS call: {}", error);
		}
		Ok(())
	}
}
