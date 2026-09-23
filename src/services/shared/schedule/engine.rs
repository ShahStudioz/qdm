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
                    && !matches!(d.state, DownloadState::Completed | DownloadState::Failed { .. })
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
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("rundll32.exe")
                        .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                        .spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("pmset")
                        .args(["sleepnow"])
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("systemctl")
                        .args(["suspend"])
                        .spawn();
                }
            }
            OnCompleteAction::ShutdownComputer => {
                #[cfg(target_os = "windows")]
                {
                    let _ = std::process::Command::new("shutdown")
                        .args([
                            "/s",
                            "/t",
                            "60",
                            "/c",
                            "QDM: All scheduled downloads complete. Computer will shut down in 60 seconds. Run 'shutdown /a' in CMD to cancel.",
                        ])
                        .spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("osascript")
                        .args(["-e", "tell app \"System Events\" to shut down"])
                        .spawn();
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("shutdown")
                        .args(["-h", "+1"])
                        .spawn();
                }
            }
        }
    }
}
