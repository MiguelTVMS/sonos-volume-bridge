//! Weekly calendar policy. Callers supply both time and zone; no system clock is read.
use jiff::{
    Timestamp,
    civil::DateTime,
    tz::{AmbiguousOffset, TimeZone},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NightModeSchedule {
    pub enabled: bool,
    pub blocks: Vec<Vec<bool>>,
}
impl Default for NightModeSchedule {
    fn default() -> Self {
        Self {
            enabled: false,
            blocks: vec![vec![false; 48]; 7],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduleWindow {
    pub active: bool,
    pub next: Option<Timestamp>,
}
impl NightModeSchedule {
    pub fn valid(&self) -> bool {
        self.blocks.len() == 7 && self.blocks.iter().all(|day| day.len() == 48)
    }
    pub fn evaluate(&self, now: Timestamp, zone: &TimeZone) -> Result<ScheduleWindow, jiff::Error> {
        if !self.enabled || !self.valid() {
            return Ok(ScheduleWindow {
                active: false,
                next: None,
            });
        }
        let date = now.to_zoned(zone.clone()).date();
        let mut boundaries = Vec::new();
        for offset in -8_i64..=8 {
            let day = date.checked_add(jiff::Span::new().days(offset))?;
            let weekday = usize::try_from(day.weekday().to_monday_zero_offset()).unwrap_or(0);
            for slot in 0..48_usize {
                let active = self.blocks[weekday][slot];
                let previous = if slot == 0 {
                    self.blocks[(weekday + 6) % 7][47]
                } else {
                    self.blocks[weekday][slot - 1]
                };
                if active == previous {
                    continue;
                }
                let hour = i8::try_from(slot / 2).unwrap_or(0);
                let minute = if slot % 2 == 0 { 0 } else { 30 };
                let stamp = resolve(day.at(hour, minute, 0, 0), zone)?;
                boundaries.push((stamp, active));
            }
        }
        // Stable ordering makes the last civil boundary win when a gap collapses a run.
        boundaries.sort_by_key(|(stamp, _)| *stamp);
        let mut collapsed: Vec<(Timestamp, bool)> = Vec::new();
        for boundary in boundaries {
            if collapsed.last().is_some_and(|last| last.0 == boundary.0) {
                collapsed.pop();
            }
            collapsed.push(boundary);
        }
        let active = collapsed
            .iter()
            .rev()
            .find(|(stamp, _)| *stamp <= now)
            .map_or(self.blocks[0][0], |(_, active)| *active);
        let next = collapsed
            .iter()
            .find(|(stamp, value)| *stamp > now && *value != active)
            .map(|(stamp, _)| *stamp);
        Ok(ScheduleWindow { active, next })
    }
}
fn resolve(mut local: DateTime, zone: &TimeZone) -> Result<Timestamp, jiff::Error> {
    loop {
        let ambiguous = zone.to_ambiguous_timestamp(local);
        if !matches!(ambiguous.offset(), AmbiguousOffset::Gap { .. }) {
            return ambiguous.earlier();
        }
        local = local.checked_add(jiff::Span::new().minutes(1))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn at(schedule: &NightModeSchedule, time: &str, zone: &str) -> ScheduleWindow {
        schedule
            .evaluate(time.parse().unwrap(), &TimeZone::get(zone).unwrap())
            .unwrap()
    }
    fn schedule() -> NightModeSchedule {
        NightModeSchedule {
            enabled: true,
            ..Default::default()
        }
    }
    #[test]
    fn exact_boundaries_and_disjoint_periods() {
        let mut s = schedule();
        s.blocks[0][2..4].fill(true);
        s.blocks[0][8] = true;
        assert!(!at(&s, "2026-09-21T00:59:59Z", "UTC").active);
        let start = at(&s, "2026-09-21T01:00:00Z", "UTC");
        assert!(start.active);
        assert_eq!(start.next.unwrap().to_string(), "2026-09-21T02:00:00Z");
        assert!(!at(&s, "2026-09-21T02:00:00Z", "UTC").active);
        assert!(at(&s, "2026-09-21T04:00:00Z", "UTC").active);
    }
    #[test]
    fn overnight_week_wrap_and_constant_weeks() {
        let mut s = schedule();
        s.blocks[6][46..].fill(true);
        s.blocks[0][..14].fill(true);
        assert_eq!(
            at(&s, "2026-09-27T23:30:00Z", "UTC")
                .next
                .unwrap()
                .to_string(),
            "2026-09-28T07:00:00Z"
        );
        for value in [false, true] {
            s.blocks.iter_mut().for_each(|d| d.fill(value));
            assert_eq!(
                at(&s, "2026-09-21T03:00:00Z", "UTC"),
                ScheduleWindow {
                    active: value,
                    next: None
                }
            );
        }
        s.enabled = false;
        assert!(!at(&s, "2026-09-21T03:00:00Z", "UTC").active);
    }
    #[test]
    fn dst_gap_collapses_and_fold_uses_first_occurrence() {
        let mut s = schedule();
        s.blocks[6][3..5].fill(true);
        assert!(!at(&s, "2026-03-29T00:59:59Z", "Europe/Lisbon").active);
        assert!(at(&s, "2026-03-29T01:00:00Z", "Europe/Lisbon").active);
        assert!(at(&s, "2026-10-25T00:30:00Z", "Europe/Lisbon").active);
        assert!(at(&s, "2026-10-25T01:00:00Z", "Europe/Lisbon").active);
        s.blocks[6].fill(false);
        s.blocks[6][2] = true;
        assert!(!at(&s, "2026-03-29T01:00:00Z", "Europe/Lisbon").active);
        assert!(!at(&s, "2026-10-25T01:00:00Z", "Europe/Lisbon").active);
    }
    #[test]
    fn invalid_dimensions_are_rejected() {
        let mut s = schedule();
        s.blocks[0].pop();
        assert!(!s.valid());
    }
}
