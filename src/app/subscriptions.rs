//! Iced subscriptions for the QDM application.
//!
//! Defines the reactive event streams that drive the application: engine
//! progress events, animation ticks, periodic timers, and network monitoring.

use super::{Message, QdmApp};
use iced::Subscription;

impl QdmApp {
    /// Returns the combined subscription that drives all background event streams.
    pub fn subscription(&self) -> Subscription<Message> {
        // --- Download engine broadcast stream ---
        let engine = self.engine.clone();
        let engine_sub = Subscription::run_with_id(
            "qdm_download_engine_stream",
            iced::stream::channel(256, move |mut output| async move {
                use iced::futures::SinkExt;
                let mut rx = engine.subscribe();
                while let Ok(event) = rx.recv().await {
                    let _ = output.send(Message::EngineEvent(event)).await;
                }
            }),
        );

        // --- Animation tick (16ms / ~60fps) — only when animating ---
        let anim_sub = if self.settings.is_animating() {
            iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick)
        } else {
            Subscription::none()
        };

        // --- 1-second tick for scheduler evaluation and conflict dialog countdown ---
        let second_tick_sub =
            iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::SecondTick);

        // --- 5-second tick for disk synchronization (detect externally deleted files) ---
        let disk_sync_sub =
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::SyncWithDisk);

        // --- 3-second network connectivity check ---
        let network_check_sub = iced::time::every(std::time::Duration::from_secs(3))
            .map(|_| Message::CheckNetworkConnectivity);

        Subscription::batch([
            engine_sub,
            anim_sub,
            second_tick_sub,
            disk_sync_sub,
            network_check_sub,
        ])
    }
}
