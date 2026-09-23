use chrono::{Datelike, NaiveDateTime, NaiveTime, Timelike, Weekday};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum OnCompleteAction {
    #[default]
    DoNothing,
    ExitQdm,
    SleepComputer,
    ShutdownComputer,
}

impl OnCompleteAction {
    pub const ALL: &'static [OnCompleteAction] = &[
        OnCompleteAction::DoNothing,
        OnCompleteAction::ExitQdm,
        OnCompleteAction::SleepComputer,
        OnCompleteAction::ShutdownComputer,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            OnCompleteAction::DoNothing => "Do nothing",
            OnCompleteAction::ExitQdm => "Exit QDM",
            OnCompleteAction::SleepComputer => "Put computer to sleep",
            OnCompleteAction::ShutdownComputer => "Turn off computer (Shut down)",
        }
    }
}

impl std::fmt::Display for OnCompleteAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

fn default_active_days() -> [bool; 7] {
    [true, true, true, true, true, true, true]
}

fn default_start_time() -> String {
    "23:00".to_string()
}

fn default_stop_time() -> String {
    "07:00".to_string()
}

fn default_true() -> bool {
    true
}

fn default_anim_for_prioritize() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(skip)]
    pub enabled_anim: f32,

    #[serde(default = "default_start_time")]
    pub start_time: String, // "HH:MM" (24h format)

    #[serde(default)]
    pub stop_enabled: bool,
    #[serde(skip)]
    pub stop_enabled_anim: f32,

    #[serde(default = "default_stop_time")]
    pub stop_time: String, // "HH:MM" (24h format)

    #[serde(default = "default_true")]
    pub prioritize_scheduled: bool,
    #[serde(skip, default = "default_anim_for_prioritize")]
    pub prioritize_scheduled_anim: f32,

    #[serde(default = "default_active_days")]
    pub active_days: [bool; 7], // 0: Mon, 1: Tue, 2: Wed, 3: Thu, 4: Fri, 5: Sat, 6: Sun

    #[serde(default)]
    pub on_complete_action: OnCompleteAction,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            enabled_anim: 0.0,
            start_time: "23:00".to_string(),
            stop_enabled: false,
            stop_enabled_anim: 0.0,
            stop_time: "07:00".to_string(),
            prioritize_scheduled: true,
            prioritize_scheduled_anim: 1.0,
            active_days: default_active_days(),
            on_complete_action: OnCompleteAction::DoNothing,
        }
    }
}

impl ScheduleConfig {
    pub fn is_day_active(&self, weekday: Weekday) -> bool {
        let idx = match weekday {
            Weekday::Mon => 0,
            Weekday::Tue => 1,
            Weekday::Wed => 2,
            Weekday::Thu => 3,
            Weekday::Fri => 4,
            Weekday::Sat => 5,
            Weekday::Sun => 6,
        };
        self.active_days[idx]
    }

    pub fn toggle_day(&mut self, day_idx: usize) {
        if day_idx < 7 {
            self.active_days[day_idx] = !self.active_days[day_idx];
        }
    }

    pub fn parse_start_time(&self) -> Option<NaiveTime> {
        parse_hh_mm(&self.start_time)
    }

    pub fn parse_stop_time(&self) -> Option<NaiveTime> {
        if self.stop_enabled {
            parse_hh_mm(&self.stop_time)
        } else {
            None
        }
    }

    pub fn formatted_start_12h(&self) -> String {
        format_12h(&self.start_time)
    }

    pub fn formatted_stop_12h(&self) -> String {
        format_12h(&self.stop_time)
    }

    /// Evaluates if the given datetime is currently inside the active scheduled download window.
    pub fn is_in_active_window(&self, now: NaiveDateTime) -> bool {
        if !self.enabled {
            return false;
        }

        let current_weekday = now.weekday();
        if !self.is_day_active(current_weekday) {
            return false;
        }

        let Some(start_t) = self.parse_start_time() else {
            return false;
        };

        let current_t = now.time();

        if let Some(stop_t) = self.parse_stop_time() {
            if start_t <= stop_t {
                // Same-day window: e.g. 14:00 to 18:00
                current_t >= start_t && current_t < stop_t
            } else {
                // Overnight window: e.g. 23:00 to 07:00
                current_t >= start_t || current_t < stop_t
            }
        } else {
            // No stop time: once start_t is reached on an active day, it is active for that day
            current_t >= start_t
        }
    }

    /// Calculates duration until the next start window begins.
    pub fn time_until_next_start(&self, now: NaiveDateTime) -> Option<std::time::Duration> {
        if !self.enabled {
            return None;
        }

        let start_t = self.parse_start_time()?;
        let current_date = now.date();

        for day_offset in 0..8 {
            let candidate_date = current_date + chrono::Days::new(day_offset);
            let candidate_weekday = candidate_date.weekday();

            if self.is_day_active(candidate_weekday) {
                let candidate_dt = NaiveDateTime::new(candidate_date, start_t);
                if candidate_dt > now {
                    let diff = candidate_dt - now;
                    return diff.to_std().ok();
                }
            }
        }

        None
    }
}

pub fn parse_hh_mm(s: &str) -> Option<NaiveTime> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() == 2 {
        let h = parts[0].trim().parse::<u32>().ok()?;
        let m = parts[1].trim().parse::<u32>().ok()?;
        if h < 24 && m < 60 {
            return NaiveTime::from_hms_opt(h, m, 0);
        }
    }
    None
}

pub fn format_12h(s: &str) -> String {
    if let Some(t) = parse_hh_mm(s) {
        let (h12, am_pm) = match t.hour() {
            0 => (12, "AM"),
            1..=11 => (t.hour(), "AM"),
            12 => (12, "PM"),
            13..=23 => (t.hour() - 12, "PM"),
            _ => (12, "AM"),
        };
        format!("{:02}:{:02} {}", h12, t.minute(), am_pm)
    } else {
        s.to_string()
    }
}

pub fn format_countdown(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else if mins > 0 {
        format!("{}m", mins)
    } else {
        format!("{}s", total_secs)
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn test_schedule_active_window_same_day() {
        let mut cfg = ScheduleConfig::default();
        cfg.enabled = true;
        cfg.start_time = "14:00".to_string();
        cfg.stop_enabled = true;
        cfg.stop_time = "18:00".to_string();

        let date = NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(); // Thursday
        let t_inside = NaiveDateTime::new(date, NaiveTime::from_hms_opt(15, 30, 0).unwrap());
        let t_before = NaiveDateTime::new(date, NaiveTime::from_hms_opt(13, 59, 0).unwrap());
        let t_after = NaiveDateTime::new(date, NaiveTime::from_hms_opt(18, 01, 0).unwrap());

        assert!(cfg.is_in_active_window(t_inside));
        assert!(!cfg.is_in_active_window(t_before));
        assert!(!cfg.is_in_active_window(t_after));
    }

    #[test]
    fn test_schedule_active_window_overnight() {
        let mut cfg = ScheduleConfig::default();
        cfg.enabled = true;
        cfg.start_time = "23:00".to_string();
        cfg.stop_enabled = true;
        cfg.stop_time = "07:00".to_string();

        let date = NaiveDate::from_ymd_opt(2026, 8, 20).unwrap(); // Thursday
        let t_night = NaiveDateTime::new(date, NaiveTime::from_hms_opt(23, 30, 0).unwrap());
        let t_morning = NaiveDateTime::new(date, NaiveTime::from_hms_opt(5, 15, 0).unwrap());
        let t_afternoon = NaiveDateTime::new(date, NaiveTime::from_hms_opt(14, 0, 0).unwrap());

        assert!(cfg.is_in_active_window(t_night));
        assert!(cfg.is_in_active_window(t_morning));
        assert!(!cfg.is_in_active_window(t_afternoon));
    }

    #[test]
    fn test_schedule_disabled_window() {
        let mut cfg = ScheduleConfig::default();
        cfg.enabled = false;
        cfg.start_time = "10:00".to_string();

        let date = NaiveDate::from_ymd_opt(2026, 8, 20).unwrap();
        let t = NaiveDateTime::new(date, NaiveTime::from_hms_opt(12, 0, 0).unwrap());
        assert!(!cfg.is_in_active_window(t));
    }

    #[test]
    fn test_schedule_formatters() {
        assert_eq!(format_12h("23:00"), "11:00 PM");
        assert_eq!(format_12h("07:30"), "07:30 AM");
        assert_eq!(format_12h("00:15"), "12:15 AM");
        assert_eq!(format_12h("12:00"), "12:00 PM");
    }
}
