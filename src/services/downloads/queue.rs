use crate::models::download::{DownloadItem, DownloadState};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct QueueSyncAction {
    pub to_start: Vec<DownloadItem>,
    pub to_pause: Vec<usize>, // IDs of downloads to pause and queue
}

pub struct QueueService;

impl QueueService {
    /// Synchronizes download states with the concurrency limit and priority order.
    /// Preempts lowest-priority resumable running downloads (excluding `protected_id` if specified),
    /// and promotes top queued downloads if slots are available.
    pub fn synchronize_queue_with_protected(
        downloads: &mut [DownloadItem],
        max_simultaneous: usize,
        protected_id: Option<usize>,
    ) -> QueueSyncAction {
        let max_simultaneous = max_simultaneous.max(1);
        let mut action = QueueSyncAction::default();

        // 1. Count active downloading items
        let mut active_indices: Vec<usize> = Vec::new();
        for (idx, item) in downloads.iter().enumerate() {
            if matches!(item.state, DownloadState::Downloading { .. }) {
                active_indices.push(idx);
            }
        }

        // 2. Preempt lowest-priority running items if concurrency exceeded
        if active_indices.len() > max_simultaneous {
            let excess = active_indices.len() - max_simultaneous;
            let mut preempted = 0;

            // Iterate in reverse (from lowest priority to highest)
            for &idx in active_indices.iter().rev() {
                if preempted >= excess {
                    break;
                }
                let item = &mut downloads[idx];
                // Do not preempt protected item
                if let Some(prot) = protected_id {
                    if item.id == prot {
                        continue;
                    }
                }
                // ONLY preempt if resumable. Non-resumable downloads must keep running!
                if item.resumable {
                    item.state = DownloadState::Queued;
                    action.to_pause.push(item.id);
                    preempted += 1;
                }
            }
        }

        // Recount active after preemption
        let current_active_count = downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Downloading { .. }))
            .count();

        // 3. Promote queued items if slots are available
        if current_active_count < max_simultaneous {
            let mut available_slots = max_simultaneous - current_active_count;

            for item in downloads.iter_mut() {
                if available_slots == 0 {
                    break;
                }

                if matches!(item.state, DownloadState::Queued) {
                    item.state = DownloadState::Downloading {
                        downloaded_bytes: item.downloaded_bytes,
                        total_bytes: item.total_bytes,
                        speed_bps: 0,
                        eta_secs: None,
                    };
                    action.to_start.push(item.clone());
                    available_slots -= 1;
                }
            }
        }

        action
    }

    /// Synchronizes download states with the concurrency limit and priority order.
    pub fn synchronize_queue(
        downloads: &mut [DownloadItem],
        max_simultaneous: usize,
    ) -> QueueSyncAction {
        Self::synchronize_queue_with_protected(downloads, max_simultaneous, None)
    }

    /// Moves a download up one position in the global queue order.
    pub fn move_item_up(downloads: &mut Vec<DownloadItem>, id: usize) -> bool {
        if let Some(pos) = downloads.iter().position(|d| d.id == id) {
            if pos > 0 {
                downloads.swap(pos, pos - 1);
                return true;
            }
        }
        false
    }

    /// Moves a download down one position in the global queue order.
    pub fn move_item_down(downloads: &mut Vec<DownloadItem>, id: usize) -> bool {
        if let Some(pos) = downloads.iter().position(|d| d.id == id) {
            if pos + 1 < downloads.len() {
                downloads.swap(pos, pos + 1);
                return true;
            }
        }
        false
    }

    /// Moves a scheduled download up relative to other scheduled downloads.
    pub fn move_scheduled_up(downloads: &mut Vec<DownloadItem>, id: usize) -> bool {
        if let Some(current_pos) = downloads.iter().position(|d| d.id == id && d.is_scheduled) {
            // Find preceding scheduled item
            let prev_scheduled_pos = downloads[..current_pos]
                .iter()
                .rposition(|d| d.is_scheduled);

            if let Some(prev_pos) = prev_scheduled_pos {
                downloads.swap(current_pos, prev_pos);
                return true;
            }
        }
        false
    }

    /// Moves a scheduled download down relative to other scheduled downloads.
    pub fn move_scheduled_down(downloads: &mut Vec<DownloadItem>, id: usize) -> bool {
        if let Some(current_pos) = downloads.iter().position(|d| d.id == id && d.is_scheduled) {
            // Find next scheduled item
            if current_pos + 1 < downloads.len() {
                let next_scheduled_pos = downloads[current_pos + 1..]
                    .iter()
                    .position(|d| d.is_scheduled)
                    .map(|offset| current_pos + 1 + offset);

                if let Some(next_pos) = next_scheduled_pos {
                    downloads.swap(current_pos, next_pos);
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::download::{DownloadUrl, FileType};

    fn make_test_item(
        id: usize,
        state: DownloadState,
        resumable: bool,
        is_scheduled: bool,
    ) -> DownloadItem {
        DownloadItem {
            id,
            filename: format!("file_{}.zip", id),
            primary_url: DownloadUrl::new("http://example.com"),
            mirror_urls: vec![],
            save_path: "/tmp".to_string(),
            downloaded_bytes: 0,
            total_bytes: Some(1000),
            state,
            file_type: FileType::Archive,
            resumable,
            is_scheduled,
            max_connections: 4,
            speed_limit_bps: None,
            etag: None,
            last_modified: None,
            sha256_hash: None,
            chunks: vec![],
            created_at: id as u64 * 100,
            updated_at: 0,
            completed_at: None,
        }
    }

    #[test]
    fn test_queue_synchronize_promotes_queued() {
        let mut downloads = vec![
            make_test_item(1, DownloadState::Queued, true, false),
            make_test_item(2, DownloadState::Queued, true, false),
            make_test_item(3, DownloadState::Queued, true, false),
        ];

        let action = QueueService::synchronize_queue(&mut downloads, 2);
        assert_eq!(action.to_start.len(), 2);
        assert_eq!(action.to_start[0].id, 1);
        assert_eq!(action.to_start[1].id, 2);
        assert!(matches!(
            downloads[0].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(
            downloads[1].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(downloads[2].state, DownloadState::Queued));
    }

    #[test]
    fn test_queue_preemption_resumable_only() {
        let mut downloads = vec![
            make_test_item(
                1,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                false,
                false,
            ), // Non-resumable
            make_test_item(
                2,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ), // Resumable
            make_test_item(
                3,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ), // Resumable
        ];

        // Max limit is 1 -> Should preempt item 3 and item 2, keeping item 1 running
        let action = QueueService::synchronize_queue(&mut downloads, 1);
        assert_eq!(action.to_pause, vec![3, 2]);
        assert!(matches!(
            downloads[0].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(downloads[1].state, DownloadState::Queued));
        assert!(matches!(downloads[2].state, DownloadState::Queued));
    }

    #[test]
    fn test_reorder_scheduled() {
        let mut downloads = vec![
            make_test_item(1, DownloadState::Queued, true, true),
            make_test_item(2, DownloadState::Queued, true, false),
            make_test_item(3, DownloadState::Queued, true, true),
        ];

        // Move item 3 up in scheduled order -> should swap with item 1
        let moved = QueueService::move_scheduled_up(&mut downloads, 3);
        assert!(moved);
        assert_eq!(downloads[0].id, 3);
        assert_eq!(downloads[1].id, 2);
        assert_eq!(downloads[2].id, 1);
    }

    #[test]
    fn test_force_resume_protected_preempts_other_running() {
        let mut downloads = vec![
            make_test_item(
                1,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ),
            make_test_item(
                2,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ),
            make_test_item(
                3,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ),
            make_test_item(
                4,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ), // User force-resumed item 4
        ];

        // Max limit is 3, item 4 is protected -> Item 3 (lowest of other running items) should be preempted
        let action = QueueService::synchronize_queue_with_protected(&mut downloads, 3, Some(4));
        assert_eq!(action.to_pause, vec![3]);
        assert!(matches!(
            downloads[0].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(
            downloads[1].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(downloads[2].state, DownloadState::Queued));
        assert!(matches!(
            downloads[3].state,
            DownloadState::Downloading { .. }
        ));
    }

    #[test]
    fn test_queue_priority_order_reversal() {
        // Items 4, 3, 2 are running, item 1 (highest priority #1) starts
        let mut downloads = vec![
            make_test_item(
                1,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ),
            make_test_item(
                2,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ),
            make_test_item(
                3,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ),
            make_test_item(
                4,
                DownloadState::Downloading {
                    downloaded_bytes: 0,
                    total_bytes: None,
                    speed_bps: 0,
                    eta_secs: None,
                },
                true,
                false,
            ),
        ];

        // Max limit is 3, normal sync -> Item 4 (lowest priority #4) should be preempted
        let action = QueueService::synchronize_queue(&mut downloads, 3);
        assert_eq!(action.to_pause, vec![4]);
        assert!(matches!(
            downloads[0].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(
            downloads[1].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(
            downloads[2].state,
            DownloadState::Downloading { .. }
        ));
        assert!(matches!(downloads[3].state, DownloadState::Queued));
    }
}
