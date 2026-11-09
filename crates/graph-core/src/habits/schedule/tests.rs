use super::*;
use chrono::DateTime;

fn time(value: &str) -> i64 { DateTime::parse_from_rfc3339(value).unwrap().timestamp() }
fn schedule(anchor: &str, every: u32, unit: IntervalUnit, mode: RecurrenceMode) -> Schedule {
    Schedule { anchor: time(anchor), every, unit, mode, timezone: None }
}

#[test]
fn late_fixed_completion_never_moves_anchor_or_accumulates_tasks() -> Result<()> {
    let s = schedule("2024-01-01T00:00:00Z", 14, IntervalUnit::Days, RecurrenceMode::Fixed);
    let day = |n: i32| s.anchor + i64::from(n) * 86400;
    let overdue = due_state(&s, None, day(60))?;
    assert_eq!(overdue, DueState { due: true, occurrence: day(56), next_occurrence: day(70) });
    assert!(!due_state(&s, Some(day(60)), day(69))?.due);
    assert!(due_state(&s, Some(day(60)), day(70))?.due);
    assert_eq!(due_state(&s, Some(day(60)), day(70))?.occurrence, day(70));
    assert!(!due_state(&s, None, day(-1))?.due);
    assert!(due_state(&s, None, day(0))?.due);
    Ok(())
}

#[test]
fn after_completion_keeps_one_overdue_deadline_then_resets_from_actual_time() -> Result<()> {
    let s = schedule("2024-01-01T00:00:00Z", 14, IntervalUnit::Days, RecurrenceMode::AfterCompletion);
    let day = |n: i32| s.anchor + i64::from(n) * 86400;
    assert_eq!(due_state(&s, None, day(60))?, DueState { due: true, occurrence: day(0), next_occurrence: day(0) });
    assert_eq!(due_state(&s, Some(day(60)), day(60))?, DueState { due: false, occurrence: day(74), next_occurrence: day(74) });
    assert!(!due_state(&s, Some(day(60)), day(74) - 1)?.due);
    assert!(due_state(&s, Some(day(60)), day(900))?.due);
    Ok(())
}

#[test]
fn fixed_months_clamp_without_drift_and_years_recover_leap_day() -> Result<()> {
    let s = schedule("2023-01-31T12:30:00Z", 1, IntervalUnit::Months, RecurrenceMode::Fixed);
    let feb = due_state(&s, None, time("2023-02-28T12:30:00Z"))?;
    assert_eq!(feb.occurrence, time("2023-02-28T12:30:00Z"));
    assert_eq!(feb.next_occurrence, time("2023-03-31T12:30:00Z"));
    assert_eq!(due_state(&s, None, time("2023-03-30T12:30:00Z"))?.occurrence, feb.occurrence);
    let leap = schedule("2020-02-29T08:00:00Z", 1, IntervalUnit::Years, RecurrenceMode::Fixed);
    assert_eq!(due_state(&leap, None, time("2023-03-01T00:00:00Z"))?.next_occurrence, time("2024-02-29T08:00:00Z"));
    let after = Schedule { mode: RecurrenceMode::AfterCompletion, ..s };
    assert_eq!(due_state(&after, Some(time("2023-02-28T12:30:00Z")), time("2023-03-01T00:00:00Z"))?.next_occurrence, time("2023-03-28T12:30:00Z"));
    Ok(())
}

#[test]
fn hours_days_weeks_are_utc_intervals_across_dst_and_offset_equivalent_inputs() -> Result<()> {
    let s = schedule("2024-03-09T12:00:00-05:00", 1, IntervalUnit::Days, RecurrenceMode::Fixed);
    assert_eq!(s.anchor, time("2024-03-09T17:00:00Z"));
    assert_eq!(due_state(&s, None, s.anchor)?.next_occurrence, time("2024-03-10T13:00:00-04:00"));
    let fall = schedule("2024-11-02T12:00:00-04:00", 1, IntervalUnit::Days, RecurrenceMode::Fixed);
    assert_eq!(due_state(&fall, None, fall.anchor)?.next_occurrence, time("2024-11-03T11:00:00-05:00"));
    for (unit, seconds) in [(IntervalUnit::Hours, 3600), (IntervalUnit::Weeks, 604800)] {
        let s = Schedule { unit, ..s.clone() };
        assert_eq!(due_state(&s, None, s.anchor)?.next_occurrence - s.anchor, seconds);
    }
    Ok(())
}

#[test]
fn invalid_and_extreme_inputs_fail_without_overflow_or_history_walks() {
    let s = schedule("0001-01-01T00:00:00Z", 1, IntervalUnit::Hours, RecurrenceMode::Fixed);
    assert!(due_state(&s, None, time("9998-12-31T00:00:00Z")).is_ok());
    for every in [0, 10001, u32::MAX] { assert!(due_state(&Schedule { every, ..s.clone() }, None, s.anchor).is_err()); }
    assert!(due_state(&s, Some(s.anchor - 1), s.anchor).is_err());
    assert!(due_state(&s, Some(s.anchor + 1), s.anchor).is_err());
    assert!(due_state(&s, None, i64::MAX).is_err());
    assert!(due_state(&Schedule { anchor: MAX_TIME, ..s }, None, MAX_TIME).is_err());
    let parsed: Schedule = serde_json::from_str(r#"{"anchor":0,"every":14,"unit":"days"}"#).unwrap();
    assert_eq!(parsed.mode, RecurrenceMode::Fixed);
}

#[test]
fn negative_confirmations_are_explicit_periods_not_unreported_success() {
    let definition = HabitDefinition { name: "Quit".into(), kind: HabitKind::Negative, schedule: None, archived: false };
    let event = EventInput { id: uuid::Uuid::new_v4(), habit_id: uuid::Uuid::new_v4(), kind: EventKind::Lapse,
        occurred_at: 100, period_start: None, period_end: None };
    assert!(validate_event(&definition, &event, 100).is_ok());
    let mut confirmation = EventInput { kind: EventKind::Confirmation, ..event.clone() };
    assert!(validate_event(&definition, &confirmation, 100).is_err());
    confirmation.period_start = Some(0); confirmation.period_end = Some(100);
    assert!(validate_event(&definition, &confirmation, 100).is_ok());
    confirmation.period_end = Some(101);
    assert!(validate_event(&definition, &confirmation, 100).is_err());
    assert!(validate_event(&definition, &EventInput { occurred_at: 101, ..event.clone() }, 100).is_err());
    assert!(validate_event(&definition, &EventInput { kind: EventKind::Completion, ..event }, 100).is_err());
}

#[test]
fn schedules_do_not_accept_non_iana_or_ambiguous_input() {
    for json in [
        r#"{"anchor":0,"every":1,"unit":"days","unexpected":true}"#,
        r#"{"anchor":0,"every":1.5,"unit":"days"}"#,
        r#"{"anchor":"2024-01-01","every":1,"unit":"days"}"#,
        r#"{"anchor":0,"every":1,"unit":"jalali_months"}"#,
    ] { assert!(serde_json::from_str::<Schedule>(json).is_err()); }
    let default_lapse: EventInput = serde_json::from_value(serde_json::json!({
        "id":uuid::Uuid::new_v4(), "habit_id":uuid::Uuid::new_v4(), "occurred_at":0,
        "period_start":null, "period_end":null,
    })).unwrap();
    assert_eq!(default_lapse.kind, EventKind::Lapse);
    let legacy: Schedule = serde_json::from_str(r#"{"anchor":0,"every":1,"unit":"days"}"#).unwrap();
    assert_eq!(legacy.timezone, None);
    assert_eq!(advance(legacy.anchor, &legacy, 1).unwrap(), 86400);
    for name in ["", "Mars/Olympus", "/etc/localtime", "../Etc/UTC", "Etc/Unknown", "EST5EDT,M3.2.0,M11.1.0", "+03:30"] {
        assert!(validate_timezone(name).is_err(), "{name}");
    }
    assert_eq!(validate_timezone("America/New_York").unwrap(), "America/New_York");
}

fn zoned(anchor: &str, every: u32, unit: IntervalUnit, mode: RecurrenceMode, name: &str) -> Schedule {
    Schedule { timezone: Some(name.into()), ..schedule(anchor, every, unit, mode) }
}

#[test]
fn daily_weekly_keep_wall_clock_across_spring_and_fall_but_hours_remain_elapsed() -> Result<()> {
    let s = zoned("2024-03-09T12:00:00-05:00", 1, IntervalUnit::Days, RecurrenceMode::Fixed, "America/New_York");
    assert_eq!(advance(s.anchor, &s, 1)?, time("2024-03-10T12:00:00-04:00"));
    assert_eq!(advance(s.anchor, &s, 1)? - s.anchor, 23 * 3600);
    let fall = zoned("2024-11-02T12:00:00-04:00", 1, IntervalUnit::Days, RecurrenceMode::Fixed, "America/New_York");
    assert_eq!(advance(fall.anchor, &fall, 1)?, time("2024-11-03T12:00:00-05:00"));
    assert_eq!(advance(fall.anchor, &fall, 1)? - fall.anchor, 25 * 3600);
    let weekly = Schedule { unit: IntervalUnit::Weeks, ..s.clone() };
    assert_eq!(advance(s.anchor, &weekly, 1)?, time("2024-03-16T12:00:00-04:00"));
    let hourly = Schedule { unit: IntervalUnit::Hours, every: 24, ..s.clone() };
    assert_eq!(advance(s.anchor, &hourly, 1)?, s.anchor + 86400);
    let completed = time("2024-03-09T15:45:00-05:00");
    let after = Schedule { mode: RecurrenceMode::AfterCompletion, ..s.clone() };
    assert_eq!(due_state(&after, Some(completed), completed)?.next_occurrence, time("2024-03-10T15:45:00-04:00"));
    assert_eq!(due_state(&s, Some(completed), completed)?.next_occurrence, time("2024-03-10T12:00:00-04:00"));
    Ok(())
}

#[test]
fn gaps_shift_by_gap_and_folds_choose_earlier_without_moving_future_fixed_times() -> Result<()> {
    let gap = zoned("2024-03-09T02:30:00-05:00", 1, IntervalUnit::Days, RecurrenceMode::Fixed, "America/New_York");
    assert_eq!(advance(gap.anchor, &gap, 1)?, time("2024-03-10T03:30:00-04:00"));
    assert_eq!(advance(gap.anchor, &gap, 2)?, time("2024-03-11T02:30:00-04:00"));
    assert_eq!(due_state(&gap, None, time("2024-03-10T03:00:00-04:00"))?.occurrence, gap.anchor);
    let fold = zoned("2024-11-02T01:30:00-04:00", 1, IntervalUnit::Days, RecurrenceMode::Fixed, "America/New_York");
    assert_eq!(advance(fold.anchor, &fold, 1)?, time("2024-11-03T01:30:00-04:00"));
    let second_fold = time("2024-11-03T01:15:00-05:00");
    assert_eq!(due_state(&fold, None, second_fold)?.occurrence, time("2024-11-03T01:30:00-04:00"));
    let later_anchor = Schedule { anchor: time("2024-11-03T01:30:00-05:00"), ..fold };
    assert_eq!(advance(later_anchor.anchor, &later_anchor, 0)?, later_anchor.anchor);
    let half_hour = zoned("2024-10-05T02:15:00+10:30", 1, IntervalUnit::Days, RecurrenceMode::Fixed, "Australia/Lord_Howe");
    assert_eq!(advance(half_hour.anchor, &half_hour, 1)?, time("2024-10-06T02:45:00+11:00"));
    Ok(())
}

#[test]
fn skipped_local_date_collapses_coincident_due_boundaries() -> Result<()> {
    let s = zoned("2011-12-29T12:00:00-10:00", 1, IntervalUnit::Days, RecurrenceMode::Fixed, "Pacific/Apia");
    let next = time("2011-12-31T12:00:00+14:00");
    assert_eq!(advance(s.anchor, &s, 1)?, next);
    assert_eq!(advance(s.anchor, &s, 2)?, next);
    let state = due_state(&s, Some(next), next)?;
    assert!(!state.due);
    assert_eq!(state.occurrence, next);
    assert_eq!(state.next_occurrence, time("2012-01-01T12:00:00+14:00"));
    Ok(())
}

#[test]
fn local_month_and_year_clamping_are_anchor_based_and_timezone_specific() -> Result<()> {
    let s = zoned("2024-01-31T18:30:00-05:00", 1, IntervalUnit::Months, RecurrenceMode::Fixed, "America/New_York");
    assert_eq!(advance(s.anchor, &s, 1)?, time("2024-02-29T18:30:00-05:00"));
    assert_eq!(advance(s.anchor, &s, 2)?, time("2024-03-31T18:30:00-04:00"));
    let y = zoned("2020-02-29T09:00:00+01:00", 1, IntervalUnit::Years, RecurrenceMode::Fixed, "Europe/Berlin");
    assert_eq!(advance(y.anchor, &y, 1)?, time("2021-02-28T09:00:00+01:00"));
    assert_eq!(advance(y.anchor, &y, 4)?, time("2024-02-29T09:00:00+01:00"));
    let explicit_utc = Schedule { timezone: Some("UTC".into()), ..s.clone() };
    let legacy = Schedule { timezone: None, ..s.clone() };
    assert_eq!(due_state(&explicit_utc, None, time("2024-04-01T00:00:00Z"))?, due_state(&legacy, None, time("2024-04-01T00:00:00Z"))?);
    Ok(())
}

#[test]
fn positive_validation_and_archival_protect_category_and_schedule() {
    let s = schedule("2024-01-01T00:00:00Z", 2, IntervalUnit::Weeks, RecurrenceMode::Fixed);
    let positive = HabitDefinition { name: "سلام".into(), kind: HabitKind::Positive, schedule: Some(s.clone()), archived: false };
    assert!(validate_definition(&positive).is_ok());
    assert!(validate_definition(&HabitDefinition { schedule: None, ..positive.clone() }).is_err());
    assert!(validate_definition(&HabitDefinition { kind: HabitKind::Negative, ..positive.clone() }).is_err());
    assert!(validate_definition(&HabitDefinition { name: " ".into(), ..positive.clone() }).is_err());
    assert!(validate_definition(&HabitDefinition { name: "😀".repeat(129), ..positive.clone() }).is_err());
    let event = EventInput { id: uuid::Uuid::new_v4(), habit_id: uuid::Uuid::new_v4(), kind: EventKind::Completion,
        occurred_at: s.anchor, period_start: None, period_end: None };
    assert!(validate_event(&positive, &event, s.anchor).is_ok());
    assert!(validate_event(&positive, &EventInput { occurred_at: s.anchor - 1, ..event.clone() }, s.anchor).is_err());
    assert!(validate_event(&HabitDefinition { archived: true, ..positive }, &event, s.anchor).is_err());
}
