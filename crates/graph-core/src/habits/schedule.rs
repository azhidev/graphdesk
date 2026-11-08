use chrono::{Datelike, Days, Months, NaiveDate, NaiveDateTime, Timelike};
use jiff::{civil, tz::TimeZone, Timestamp};
use crate::{Error, Result};
use super::*;

const MIN_TIME: i64 = -62_135_596_800; // 0001-01-01T00:00:00Z
const MAX_TIME: i64 = 253_402_300_799; // 9999-12-31T23:59:59Z
fn invalid(message: &str) -> Error { Error::Invalid(message.into()) }

/// Only database-backed names, never paths, offsets, POSIX rules or unknown UTC
/// fallbacks. Jiff already exists in the lockfile; its TZ database is cached.
pub fn validate_timezone(name: &str) -> Result<String> {
    if name.is_empty() || name.len() > 100 || name.starts_with('/')
        || name.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"/_-+".contains(&b)) {
        return Err(invalid("habit timezone must be an IANA database name"));
    }
    TimeZone::get(name).ok().and_then(|zone| zone.iana_name().map(str::to_owned))
        .ok_or_else(|| invalid("habit timezone is unavailable; choose a supported IANA timezone"))
}

pub fn system_timezone() -> Result<String> {
    let zone = TimeZone::try_system().map_err(|_| invalid("system timezone unavailable; choose a habit timezone in Settings"))?;
    let name = zone.iana_name().ok_or_else(|| invalid("system timezone has no IANA identity; choose one in Settings"))?;
    validate_timezone(name)
}

/// The core never reads app preferences. Direct core creates default to UTC;
/// the adapter supplies the resolved creation default or existing update zone.
pub fn materialize_timezone(definition: &mut HabitDefinition, default: &str) -> Result<()> {
    if let Some(schedule) = &mut definition.schedule {
        schedule.timezone = Some(validate_timezone(schedule.timezone.as_deref().unwrap_or(default))?);
    }
    Ok(())
}

fn zone(schedule: &Schedule) -> Result<TimeZone> {
    let name = validate_timezone(schedule.timezone.as_deref().unwrap_or("UTC"))?;
    TimeZone::get(&name).map_err(|_| invalid("habit timezone database lookup failed"))
}

pub fn validate_time(value: i64) -> Result<()> {
    if !(MIN_TIME..=MAX_TIME).contains(&value) { return Err(invalid("habit time outside supported UTC years 1..9999")); }
    Ok(())
}

pub fn validate_definition(value: &HabitDefinition) -> Result<()> {
    if value.name.trim().is_empty() || value.name.len() > 512 || value.name.contains('\0') {
        return Err(invalid("habit name must contain text and fit 512 UTF-8 bytes"));
    }
    match (value.kind, &value.schedule) {
        (HabitKind::Positive, Some(schedule)) => validate_schedule(schedule),
        (HabitKind::Negative, None) => Ok(()),
        _ => Err(invalid("positive habits require a schedule; negative habits have no recurrence or reminders")),
    }
}

fn validate_schedule(schedule: &Schedule) -> Result<()> {
    validate_time(schedule.anchor)?;
    zone(schedule)?;
    if schedule.every == 0 || schedule.every > 10000 { return Err(invalid("habit interval must be 1..10000")); }
    advance(schedule.anchor, schedule, 1)?;
    Ok(())
}

pub fn validate_event(definition: &HabitDefinition, event: &EventInput, recorded_at: i64) -> Result<()> {
    validate_definition(definition)?;
    validate_time(event.occurred_at)?; validate_time(recorded_at)?;
    if definition.archived { return Err(invalid("archived habits cannot receive new events")); }
    if event.id.is_nil() || event.habit_id.is_nil() { return Err(invalid("habit and event IDs must not be nil")); }
    if event.occurred_at > recorded_at { return Err(invalid("habit event cannot occur after server recording time")); }
    match (definition.kind, event.kind) {
        (HabitKind::Positive, EventKind::Completion) => {
            if event.period_start.is_some() || event.period_end.is_some() { return Err(invalid("completion events do not declare periods")); }
            if event.occurred_at < definition.schedule.as_ref().unwrap().anchor { return Err(invalid("completion precedes schedule anchor")); }
        },
        (HabitKind::Negative, EventKind::Lapse) => {
            if event.period_start.is_some() || event.period_end.is_some() { return Err(invalid("lapse events do not declare successful periods")); }
        },
        (HabitKind::Negative, EventKind::Confirmation) => {
            let (Some(start), Some(end)) = (event.period_start, event.period_end) else {
                return Err(invalid("success confirmation requires an explicit period"));
            };
            validate_time(start)?; validate_time(end)?;
            if start >= end || end > event.occurred_at { return Err(invalid("confirmation must describe a completed nonempty [start,end) period")); }
        },
        _ => return Err(invalid("event kind does not match habit category")),
    }
    Ok(())
}

fn local_time(base: i64, zone: &TimeZone) -> Result<NaiveDateTime> {
    let instant = Timestamp::from_second(base).map_err(|_| invalid("habit time outside supported range"))?;
    let value = zone.to_datetime(instant);
    if !(1..=9999).contains(&value.year()) { return Err(invalid("habit local time outside supported years 1..9999")); }
    NaiveDate::from_ymd_opt(i32::from(value.year()), value.month() as u32, value.day() as u32)
        .and_then(|date| date.and_hms_opt(value.hour() as u32, value.minute() as u32, value.second() as u32))
        .ok_or_else(|| invalid("invalid habit local time"))
}

fn resolve_local(value: NaiveDateTime, zone: &TimeZone) -> Result<i64> {
    if !(1..=9999).contains(&value.year()) { return Err(invalid("habit local time outside supported years 1..9999")); }
    let value = civil::DateTime::new(value.year() as i16, value.month() as i8, value.day() as i8,
        value.hour() as i8, value.minute() as i8, value.second() as i8, 0)
        .map_err(|_| invalid("invalid habit local calendar boundary"))?;
    // Temporal-compatible: earlier instant on a fold; shift forward by the
    // exact gap on a nonexistent time (including non-hour and skipped-date gaps).
    let result = zone.to_ambiguous_zoned(value).compatible()
        .map_err(|_| invalid("habit timezone boundary outside supported range"))?.timestamp().as_second();
    validate_time(result)?;
    Ok(result)
}

/// Always start from the intended base wall time, not a prior DST-adjusted or
/// clamped occurrence. This prevents both DST-gap and month-end drift.
fn advance(base: i64, schedule: &Schedule, periods: u64) -> Result<i64> {
    if periods == 0 { return Ok(base); } // Preserve an explicitly anchored fold instant.
    let amount = u64::from(schedule.every).checked_mul(periods).ok_or_else(|| invalid("habit interval overflow"))?;
    let result = if schedule.unit == IntervalUnit::Hours {
        let delta = amount.checked_mul(3600).and_then(|v| i64::try_from(v).ok()).ok_or_else(|| invalid("habit interval overflow"))?;
        base.checked_add(delta).ok_or_else(|| invalid("habit interval overflow"))?
    } else {
        let tz = zone(schedule)?;
        let local = local_time(base, &tz)?;
        let target = match schedule.unit {
            IntervalUnit::Days | IntervalUnit::Weeks => {
                let days = amount.checked_mul(if schedule.unit == IntervalUnit::Weeks { 7 } else { 1 })
                    .ok_or_else(|| invalid("habit calendar interval overflow"))?;
                local.checked_add_days(Days::new(days))
            },
            _ => {
                let months = amount.checked_mul(if schedule.unit == IntervalUnit::Years { 12 } else { 1 })
                    .and_then(|v| u32::try_from(v).ok()).ok_or_else(|| invalid("habit calendar interval overflow"))?;
                local.checked_add_months(Months::new(months))
            },
        }.ok_or_else(|| invalid("habit calendar boundary outside supported range"))?;
        resolve_local(target, &tz)?
    };
    validate_time(result)?;
    Ok(result)
}

/// Pure calculation: explicit now and latest nonvoided completion, no wall clock,
/// implicit device timezone, task generation, history scans or backlog tasks.
pub fn due_state(schedule: &Schedule, latest_completion: Option<i64>, now: i64) -> Result<DueState> {
    validate_schedule(schedule)?; validate_time(now)?;
    if let Some(completion) = latest_completion {
        validate_time(completion)?;
        if completion < schedule.anchor || completion > now { return Err(invalid("completion outside evaluated schedule lifetime")); }
    }
    if now < schedule.anchor {
        return Ok(DueState { due: false, occurrence: schedule.anchor, next_occurrence: schedule.anchor });
    }
    if schedule.mode == RecurrenceMode::AfterCompletion {
        let deadline = match latest_completion { Some(time) => advance(time, schedule, 1)?, None => schedule.anchor };
        return Ok(DueState { due: now >= deadline, occurrence: deadline, next_occurrence: deadline });
    }
    let period = if schedule.unit == IntervalUnit::Hours {
        ((now - schedule.anchor) / (i64::from(schedule.every) * 3600)) as u64
    } else {
        // Boundaries are nondecreasing (a skipped date can coincide with the
        // following boundary). Find the LAST boundary <= now, collapsing ties.
        // Range errors bound the search; at most ~2*log2(3.7M) calendar probes.
        let (mut low, mut high) = (0_u64, 1_u64);
        while advance(schedule.anchor, schedule, high).is_ok_and(|at| at <= now) {
            low = high; high *= 2;
        }
        while high - low > 1 {
            let middle = low + (high - low) / 2;
            if advance(schedule.anchor, schedule, middle).is_ok_and(|at| at <= now) { low = middle; }
            else { high = middle; }
        }
        low
    };
    let occurrence = advance(schedule.anchor, schedule, period)?;
    let next_occurrence = advance(schedule.anchor, schedule, period + 1)?;
    Ok(DueState { due: latest_completion.is_none_or(|completion| completion < occurrence), occurrence, next_occurrence })
}

#[cfg(test)]
mod tests;
