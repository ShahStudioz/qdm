use crate::models::download::{DownloadItem, DownloadState};
use crate::models::schedule::{OnCompleteAction, ScheduleConfig};
use chrono::Local;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchedulerTickAction {
    None,
    ResumeDownloads(Vec<usize>), // IDs of items to start/resume
    PauseDownloads(Vec<usize>),  // IDs of items to pause back into Scheduled state
    TriggerPowerAction(OnCompleteAction),
}

pub struct SchedulerService;

impl SchedulerService {
    /// Evaluates scheduled download states and returns actions to take on this tick.
    pub fn evaluate_tick(
        config: &ScheduleConfig,
        downloads: &[DownloadItem],
        power_action_triggered: bool,
    ) -> SchedulerTickAction {
        if !config.enabled {
            return SchedulerTickAction::None;
        }

        let now = Local::now().naive_local();
        let in_window = config.is_in_active_window(now);

        if in_window {
            // Find any scheduled downloads waiting to start (in Scheduled state)
            let mut to_resume = Vec::new();
            for item in downloads {
                if item.is_scheduled && matches!(item.state, DownloadState::Scheduled) {
                    to_resume.push(item.id);
                }
            }

            if !to_resume.is_empty() {
                return SchedulerTickAction::ResumeDownloads(to_resume);
            }

            // Check if there are active scheduled downloads still running or queued
            let has_active_scheduled = downloads.iter().any(|d| {
                d.is_scheduled
                    && !matches!(
                        d.state,
                        DownloadState::Completed | DownloadState::Failed { .. }
                    )
            });

            // If all scheduled downloads are finished and we haven't triggered the power action yet
            if !has_active_scheduled
                && !power_action_triggered
                && config.on_complete_action != OnCompleteAction::DoNothing
            {
                let has_completed = downloads
                    .iter()
                    .any(|d| d.is_scheduled && matches!(d.state, DownloadState::Completed));
                if has_completed {
                    return SchedulerTickAction::TriggerPowerAction(config.on_complete_action);
                }
            }
        } else if config.stop_enabled {
            // Outside window and stop is enabled: pause any active/queued scheduled downloads back to Scheduled
            let mut to_pause = Vec::new();
            for item in downloads {
                if item.is_scheduled
                    && matches!(
                        item.state,
                        DownloadState::Downloading { .. }
                            | DownloadState::Queued
                            | DownloadState::WaitingForNetwork { .. }
                            | DownloadState::FetchingMetadata
                    )
                {
                    to_pause.push(item.id);
                }
            }

            if !to_pause.is_empty() {
                return SchedulerTickAction::PauseDownloads(to_pause);
            }
        }

        SchedulerTickAction::None
    }

    /// Executes the configured post-completion action safely.
    pub fn execute_power_action(action: OnCompleteAction) {
        println!(
            "[QDM Scheduler] Executing post-completion action: {:?}",
            action
        );
        match action {
            OnCompleteAction::DoNothing => {}
            OnCompleteAction::ExitQdm => {
                std::process::exit(0);
            }
            OnCompleteAction::SleepComputer => {
                if let Err(e) = crate::core::utils::platform::sleep_computer() {
                    eprintln!("[QDM Scheduler] Failed to sleep computer: {}", e);
                }
            }
            OnCompleteAction::ShutdownComputer => {
                if let Err(e) = crate::core::utils::platform::shutdown_computer() {
                    eprintln!("[QDM Scheduler] Failed to shutdown computer: {}", e);
                }
            }
        }
    }
}
