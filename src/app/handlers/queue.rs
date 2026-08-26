//! Handles queue ordering and download scheduler evaluation.
//!
//! This module processes queue item reordering (move up/down), scheduled
//! download management, and the per-second scheduler tick that evaluates
//! whether to start, pause, or trigger power actions for scheduled downloads.

use crate::app::{Message, QdmApp};
use crate::models::download::DownloadState;
use crate::services::storage;
use crate::views::dialogues::conflict_dialogue;
use iced::Task;

/// Handles the 1-second tick: evaluates the conflict dialog countdown and
/// the download scheduler's active window.
pub(crate) fn handle_second_tick(app: &mut QdmApp) -> Task<Message> {
    // 1. Conflict dialog auto-action countdown
    if app.conflict_dialog.is_open
        && app.conflict_dialog.tick_second()
    {
        return app.update(Message::ConflictDialogueMessages(
            conflict_dialogue::ConflictDialogMessage::AutoRenameChosen,
        ));
    }

    // 2. Automated Download Scheduler evaluation
    let action = crate::services::schedule::SchedulerService::evaluate_tick(
        &app.settings.schedule,
        &app.downloads,
        app.schedule_power_action_triggered,
    );

    match action {
        crate::services::schedule::SchedulerTickAction::ResumeDownloads(ids) => {
            for id in ids {
                if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
                    println!(
                        "[QDM Scheduler] Pushing scheduled download {} into queue for active window",
                        id
                    );
                    if matches!(item.state, DownloadState::Scheduled) {
                        item.state = DownloadState::Queued;
                    }
                }
            }
            app.synchronize_and_persist_queue()
        }
        crate::services::schedule::SchedulerTickAction::PauseDownloads(ids) => {
            let mut tasks = Vec::new();
            for id in ids {
                if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
                    println!(
                        "[QDM Scheduler] Auto-pausing download {} (schedule window ended)",
                        id
                    );
                    item.state = DownloadState::Scheduled;
                    let engine = app.engine.clone();
                    tasks.push(Task::perform(
                        async move {
                            engine.pause(id).await;
                            Ok(())
                        },
                        |_: Result<(), String>| Message::Tick,
                    ));
                }
            }
            if !tasks.is_empty() {
                let downloads_clone = app.downloads.clone();
                return Task::batch([
                    Task::perform(
                        async move { storage::json_store::save_downloads(&downloads_clone) },
                        Message::DownloadsPersisted,
                    ),
                    Task::batch(tasks),
                ]);
            }
            Task::none()
        }
        crate::services::schedule::SchedulerTickAction::TriggerPowerAction(power_action) => {
            app.schedule_power_action_triggered = true;
            crate::services::schedule::SchedulerService::execute_power_action(power_action);
            Task::none()
        }
        crate::services::schedule::SchedulerTickAction::None => Task::none(),
    }
}

/// Moves a queued download one position up in the queue, then re-synchronizes.
pub(crate) fn handle_move_queue_item_up(app: &mut QdmApp, id: usize) -> Task<Message> {
    if crate::services::downloads::QueueService::move_item_up(&mut app.downloads, id) {
        return app.synchronize_and_persist_queue();
    }
    Task::none()
}

/// Moves a queued download one position down in the queue, then re-synchronizes.
pub(crate) fn handle_move_queue_item_down(app: &mut QdmApp, id: usize) -> Task<Message> {
    if crate::services::downloads::QueueService::move_item_down(&mut app.downloads, id) {
        return app.synchronize_and_persist_queue();
    }
    Task::none()
}

/// Moves a scheduled download one position up in the schedule list.
pub(crate) fn handle_move_scheduled_item_up(app: &mut QdmApp, id: usize) -> Task<Message> {
    if crate::services::downloads::QueueService::move_scheduled_up(&mut app.downloads, id) {
        let downloads_clone = app.downloads.clone();
        return Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}

/// Moves a scheduled download one position down in the schedule list.
pub(crate) fn handle_move_scheduled_item_down(app: &mut QdmApp, id: usize) -> Task<Message> {
    if crate::services::downloads::QueueService::move_scheduled_down(&mut app.downloads, id) {
        let downloads_clone = app.downloads.clone();
        return Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}

/// Removes a download from the scheduler (keeps the download, clears schedule flag).
pub(crate) fn handle_remove_from_schedule(app: &mut QdmApp, id: usize) -> Task<Message> {
    if let Some(item) = app.downloads.iter_mut().find(|d| d.id == id) {
        item.is_scheduled = false;
        let downloads_clone = app.downloads.clone();
        return Task::perform(
            async move { storage::json_store::save_downloads(&downloads_clone) },
            Message::DownloadsPersisted,
        );
    }
    Task::none()
}
