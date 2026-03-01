use anyhow::{bail, Result};

const DOW_NAMES: &[&str] = &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DOW_FULL_NAMES: &[&str] = &[
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

#[derive(Debug)]
pub struct CronSchedule {
    pub on_calendar: Option<String>,
    pub on_boot_sec: Option<String>,
    pub is_service: bool,
    /// Normalized display form (e.g., "@daily/9", "@monday/9:30")
    pub display: Option<String>,
}

pub fn parse(expr: &str) -> Result<CronSchedule> {
    let trimmed = expr.trim();

    // Try extended syntax first (e.g., @daily/9, @monday/9:30, @1st/8)
    if let Some(schedule) = parse_extended(trimmed)? {
        return Ok(schedule);
    }

    // Try classic special expressions (e.g., @daily, @weekly)
    if let Some(special) = parse_special(trimmed) {
        return Ok(special);
    }

    // Check for incomplete extended syntax and give helpful errors
    if trimmed.starts_with('@') {
        check_incomplete_syntax(trimmed)?;
    }

    // Fall back to standard cron 5-field format
    let fields: Vec<&str> = trimmed.split_whitespace().collect();
    if fields.len() != 5 {
        bail!("Invalid cron expression: expected 5 fields, got {}", fields.len());
    }

    let minute = parse_field(fields[0], FieldType::Minute)?;
    let hour = parse_field(fields[1], FieldType::Hour)?;
    let dom = parse_field(fields[2], FieldType::DayOfMonth)?;
    let month = parse_field(fields[3], FieldType::Month)?;
    let dow = parse_dow_field(fields[4])?;

    let mut calendar = String::new();

    if let Some(ref dow_str) = dow {
        calendar.push_str(dow_str);
        calendar.push(' ');
    }

    // Date part: *-MM-DD
    calendar.push_str(&format!("*-{}-{}", month, dom));

    // Time part: HH:MM:SS
    calendar.push_str(&format!(" {}:{}:00", hour, minute));

    Ok(CronSchedule {
        on_calendar: Some(calendar),
        on_boot_sec: None,
        is_service: false,
        display: None,
    })
}

fn parse_special(expr: &str) -> Option<CronSchedule> {
    match expr {
        "@yearly" | "@annually" => Some(CronSchedule {
            on_calendar: Some("*-01-01 00:00:00".to_string()),
            on_boot_sec: None,
            is_service: false,
            display: Some("@yearly".to_string()),
        }),
        "@monthly" => Some(CronSchedule {
            on_calendar: Some("*-*-01 00:00:00".to_string()),
            on_boot_sec: None,
            is_service: false,
            display: Some("@monthly".to_string()),
        }),
        "@weekly" => Some(CronSchedule {
            on_calendar: Some("Mon *-*-* 00:00:00".to_string()),
            on_boot_sec: None,
            is_service: false,
            display: Some("@weekly".to_string()),
        }),
        "@daily" | "@midnight" => Some(CronSchedule {
            on_calendar: Some("*-*-* 00:00:00".to_string()),
            on_boot_sec: None,
            is_service: false,
            display: Some("@daily".to_string()),
        }),
        "@hourly" => Some(CronSchedule {
            on_calendar: Some("*-*-* *:00:00".to_string()),
            on_boot_sec: None,
            is_service: false,
            display: Some("@hourly".to_string()),
        }),
        "@reboot" => Some(CronSchedule {
            on_calendar: None,
            on_boot_sec: Some("1min".to_string()),
            is_service: false,
            display: Some("@reboot".to_string()),
        }),
        "@service" => Some(CronSchedule {
            on_calendar: None,
            on_boot_sec: None,
            is_service: true,
            display: Some("@service".to_string()),
        }),
        _ => None,
    }
}

/// Check for incomplete or invalid extended syntax and provide helpful errors
fn check_incomplete_syntax(expr: &str) -> Result<()> {
    let lower = expr.to_lowercase();

    // Check for weekday without time: @monday, @tue, etc.
    if is_weekday_keyword(&lower) {
        bail!(
            "Missing time for '{}'. Use: {}/9 or {}/9:30",
            expr, expr, expr
        );
    }

    // Check for out-of-range ordinal: @32nd, @0th, etc.
    if let Some(day) = try_parse_ordinal_any(&lower) {
        if !(1..=31).contains(&day) {
            bail!("Day must be 1-31, got {}", day);
        }
        // Valid ordinal but missing time
        bail!(
            "Missing time for '{}'. Use: {}/9 or {}/9:30",
            expr, expr, expr
        );
    }

    // Check for @daily, @weekly, @monthly without proper format
    if lower == "@daily" || lower == "@weekly" || lower == "@monthly" {
        // These are handled by parse_special, so no error needed here
        return Ok(());
    }

    Ok(())
}

/// Check if keyword is a weekday name (without @)
fn is_weekday_keyword(keyword: &str) -> bool {
    let name = match keyword.strip_prefix('@') {
        Some(n) => n,
        None => return false,
    };
    for &full in DOW_FULL_NAMES {
        if full == name || (name.len() >= 3 && full.starts_with(name)) {
            return true;
        }
    }
    for &short in DOW_NAMES {
        if short.to_lowercase() == name {
            return true;
        }
    }
    false
}

/// Parse ordinal without range check (for error messages)
fn try_parse_ordinal_any(keyword: &str) -> Option<u32> {
    let s = keyword.strip_prefix('@')?;
    let num_str = s
        .strip_suffix("st")
        .or_else(|| s.strip_suffix("nd"))
        .or_else(|| s.strip_suffix("rd"))
        .or_else(|| s.strip_suffix("th"))?;
    num_str.parse().ok()
}

/// Parse extended schedule syntax:
/// - @daily/HH or @daily/HH:MM
/// - @monday/HH, @mon/HH (weekday names)
/// - @weekly/dow/HH (alternative weekly syntax)
/// - @1st/HH, @2nd/HH, etc. (monthly by ordinal)
/// - @monthly/DD/HH (alternative monthly syntax)
fn parse_extended(expr: &str) -> Result<Option<CronSchedule>> {
    if !expr.starts_with('@') || !expr.contains('/') {
        return Ok(None);
    }

    let parts: Vec<&str> = expr.split('/').collect();
    if parts.is_empty() {
        return Ok(None);
    }

    let keyword = parts[0].to_lowercase();

    // @daily/HH or @daily/HH:MM
    if keyword == "@daily" {
        if parts.len() != 2 {
            bail!("Invalid @daily syntax. Use: @daily/9 or @daily/9:30");
        }
        let (hour, minute) = parse_time_spec(parts[1])?;
        let display = format_time_display("@daily", hour, minute);
        return Ok(Some(CronSchedule {
            on_calendar: Some(format!("*-*-* {:02}:{:02}:00", hour, minute)),
            on_boot_sec: None,
            is_service: false,
            display: Some(display),
        }));
    }

    // @weekly/dow/HH or @weekly/dow/HH:MM
    if keyword == "@weekly" {
        if parts.len() != 3 {
            bail!("Invalid @weekly syntax. Use: @weekly/mon/9 or @weekly/mon/9:30");
        }
        let dow = parse_dow_name(parts[1])?;
        let (hour, minute) = parse_time_spec(parts[2])?;
        let display = format!("@{}/{}", dow.to_lowercase(), format_time(hour, minute));
        return Ok(Some(CronSchedule {
            on_calendar: Some(format!("{} *-*-* {:02}:{:02}:00", dow, hour, minute)),
            on_boot_sec: None,
            is_service: false,
            display: Some(display),
        }));
    }

    // @monthly/DD/HH or @monthly/DD/HH:MM
    if keyword == "@monthly" {
        if parts.len() != 3 {
            bail!("Invalid @monthly syntax. Use: @monthly/1/9 or @monthly/15/9:30");
        }
        let day: u32 = parts[1].parse().map_err(|_| anyhow::anyhow!("Invalid day: {}", parts[1]))?;
        if !(1..=31).contains(&day) {
            bail!("Day must be between 1 and 31");
        }
        let (hour, minute) = parse_time_spec(parts[2])?;
        let display = format!("@{}/{}", ordinal(day), format_time(hour, minute));
        return Ok(Some(CronSchedule {
            on_calendar: Some(format!("*-*-{:02} {:02}:{:02}:00", day, hour, minute)),
            on_boot_sec: None,
            is_service: false,
            display: Some(display),
        }));
    }

    // @monday/HH, @tue/HH:MM, etc. (weekday names directly)
    if let Some(dow) = try_parse_dow_keyword(&keyword) {
        if parts.len() != 2 {
            bail!("Invalid weekday syntax. Use: @monday/9 or @mon/9:30");
        }
        let (hour, minute) = parse_time_spec(parts[1])?;
        let display = format!("@{}/{}", dow.to_lowercase(), format_time(hour, minute));
        return Ok(Some(CronSchedule {
            on_calendar: Some(format!("{} *-*-* {:02}:{:02}:00", dow, hour, minute)),
            on_boot_sec: None,
            is_service: false,
            display: Some(display),
        }));
    }

    // @1st/HH, @2nd/HH:MM, etc. (ordinal day of month)
    if let Some(day) = try_parse_ordinal(&keyword) {
        if parts.len() != 2 {
            bail!("Invalid ordinal syntax. Use: @1st/9 or @15th/9:30");
        }
        let (hour, minute) = parse_time_spec(parts[1])?;
        let display = format!("@{}/{}", ordinal(day), format_time(hour, minute));
        return Ok(Some(CronSchedule {
            on_calendar: Some(format!("*-*-{:02} {:02}:{:02}:00", day, hour, minute)),
            on_boot_sec: None,
            is_service: false,
            display: Some(display),
        }));
    }

    // Check for out-of-range ordinal (e.g., @32nd/9, @0th/9)
    if let Some(day) = try_parse_ordinal_any(&keyword) {
        bail!("Day must be 1-31, got {}", day);
    }

    Ok(None)
}

/// Parse time specification: "9" -> (9, 0), "9:30" -> (9, 30)
fn parse_time_spec(s: &str) -> Result<(u32, u32)> {
    if let Some((h, m)) = s.split_once(':') {
        let hour: u32 = h.parse().map_err(|_| anyhow::anyhow!("Invalid hour: {}", h))?;
        let minute: u32 = m.parse().map_err(|_| anyhow::anyhow!("Invalid minute: {}", m))?;
        if hour > 23 {
            bail!("Hour must be 0-23");
        }
        if minute > 59 {
            bail!("Minute must be 0-59");
        }
        Ok((hour, minute))
    } else {
        let hour: u32 = s.parse().map_err(|_| anyhow::anyhow!("Invalid hour: {}", s))?;
        if hour > 23 {
            bail!("Hour must be 0-23");
        }
        Ok((hour, 0))
    }
}

/// Parse weekday name from @keyword (e.g., "@monday" -> "Mon")
fn try_parse_dow_keyword(keyword: &str) -> Option<&'static str> {
    let name = keyword.strip_prefix('@')?;
    for (i, &full) in DOW_FULL_NAMES.iter().enumerate() {
        if full == name || full.starts_with(name) {
            return Some(DOW_NAMES[i]);
        }
    }
    // Also check short names
    for (i, &short) in DOW_NAMES.iter().enumerate() {
        if short.to_lowercase() == name {
            return Some(DOW_NAMES[i]);
        }
    }
    None
}

/// Parse ordinal suffix (e.g., "@1st" -> 1, "@22nd" -> 22)
fn try_parse_ordinal(keyword: &str) -> Option<u32> {
    let s = keyword.strip_prefix('@')?;
    let num_str = s
        .strip_suffix("st")
        .or_else(|| s.strip_suffix("nd"))
        .or_else(|| s.strip_suffix("rd"))
        .or_else(|| s.strip_suffix("th"))?;
    let n: u32 = num_str.parse().ok()?;
    if (1..=31).contains(&n) {
        Some(n)
    } else {
        None
    }
}

/// Parse dow name (e.g., "mon" -> "Mon", "monday" -> "Mon")
fn parse_dow_name(s: &str) -> Result<&'static str> {
    let lower = s.to_lowercase();
    for (i, &full) in DOW_FULL_NAMES.iter().enumerate() {
        if full == lower || full.starts_with(&lower) {
            return Ok(DOW_NAMES[i]);
        }
    }
    for (i, &short) in DOW_NAMES.iter().enumerate() {
        if short.to_lowercase() == lower {
            return Ok(DOW_NAMES[i]);
        }
    }
    bail!("Invalid day of week: {}", s)
}

/// Format ordinal (1 -> "1st", 2 -> "2nd", etc.)
fn ordinal(n: u32) -> String {
    let suffix = match n % 10 {
        1 if n % 100 != 11 => "st",
        2 if n % 100 != 12 => "nd",
        3 if n % 100 != 13 => "rd",
        _ => "th",
    };
    format!("{}{}", n, suffix)
}

/// Format time for display (9, 0) -> "9", (9, 30) -> "9:30"
fn format_time(hour: u32, minute: u32) -> String {
    if minute == 0 {
        format!("{}", hour)
    } else {
        format!("{}:{:02}", hour, minute)
    }
}

/// Format display string with time
fn format_time_display(prefix: &str, hour: u32, minute: u32) -> String {
    format!("{}/{}", prefix, format_time(hour, minute))
}

#[derive(Clone, Copy)]
enum FieldType {
    Minute,
    Hour,
    DayOfMonth,
    Month,
}

impl FieldType {
    fn range(self) -> (u32, u32) {
        match self {
            FieldType::Minute => (0, 59),
            FieldType::Hour => (0, 23),
            FieldType::DayOfMonth => (1, 31),
            FieldType::Month => (1, 12),
        }
    }

    fn start(self) -> u32 {
        self.range().0
    }
}

fn parse_field(field: &str, field_type: FieldType) -> Result<String> {
    if field == "*" {
        return Ok("*".to_string());
    }

    // Handle comma-separated list
    let parts: Vec<&str> = field.split(',').collect();
    if parts.len() > 1 {
        let mut values: Vec<String> = Vec::new();
        for part in parts {
            values.push(parse_single_element(part, field_type)?);
        }
        return Ok(values.join(","));
    }

    parse_single_element(field, field_type)
}

fn parse_single_element(element: &str, field_type: FieldType) -> Result<String> {
    // */N
    if let Some(step_str) = element.strip_prefix("*/") {
        let step: u32 = step_str.parse()?;
        let start = field_type.start();
        return Ok(format!("{}/{}", start, step));
    }

    // N-M/S (range with step → expand to comma list)
    if element.contains('-') && element.contains('/') {
        let (range_part, step_str) = element.split_once('/').unwrap();
        let step: u32 = step_str.parse()?;
        let (start_str, end_str) = range_part.split_once('-').unwrap();
        let start: u32 = start_str.parse()?;
        let end: u32 = end_str.parse()?;
        let mut values = Vec::new();
        let mut v = start;
        while v <= end {
            values.push(format!("{:0>2}", v));
            v += step;
        }
        return Ok(values.join(","));
    }

    // N-M (range)
    if let Some((start_str, end_str)) = element.split_once('-') {
        let start: u32 = start_str.parse()?;
        let end: u32 = end_str.parse()?;
        return Ok(format!("{}..{}", start, end));
    }

    // Plain number
    let n: u32 = element.parse()?;
    Ok(format!("{:0>2}", n))
}

fn parse_dow_field(field: &str) -> Result<Option<String>> {
    if field == "*" {
        return Ok(None);
    }

    let parts: Vec<&str> = field.split(',').collect();
    if parts.len() > 1 {
        let mut names: Vec<String> = Vec::new();
        for part in parts {
            names.push(parse_single_dow(part)?);
        }
        return Ok(Some(names.join(",")));
    }

    Ok(Some(parse_single_dow(field)?))
}

fn parse_single_dow(element: &str) -> Result<String> {
    // Range: N-M
    if let Some((start_str, end_str)) = element.split_once('-') {
        let start = dow_to_name(start_str)?;
        let end = dow_to_name(end_str)?;
        return Ok(format!("{}..{}", start, end));
    }

    dow_to_name(element)
}

fn dow_to_name(s: &str) -> Result<String> {
    // Accept both numeric (0-7) and name abbreviations
    if let Ok(n) = s.parse::<u32>() {
        let idx = if n == 7 { 0 } else { n as usize };
        if idx < DOW_NAMES.len() {
            return Ok(DOW_NAMES[idx].to_string());
        }
        bail!("Invalid day of week number: {}", n);
    }

    // Try matching name abbreviations (case-insensitive)
    let lower = s.to_lowercase();
    for &name in DOW_NAMES {
        if name.to_lowercase() == lower || name.to_lowercase().starts_with(&lower) {
            return Ok(name.to_string());
        }
    }

    bail!("Invalid day of week: {}", s);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cal(expr: &str) -> String {
        let result = parse(expr).unwrap();
        result.on_calendar.unwrap()
    }

    fn display(expr: &str) -> String {
        let result = parse(expr).unwrap();
        result.display.unwrap_or_default()
    }

    #[test]
    fn every_minute() {
        assert_eq!(cal("* * * * *"), "*-*-* *:*:00");
    }

    #[test]
    fn daily_at_9() {
        assert_eq!(cal("0 9 * * *"), "*-*-* 09:00:00");
    }

    #[test]
    fn every_5_minutes() {
        assert_eq!(cal("*/5 * * * *"), "*-*-* *:0/5:00");
    }

    #[test]
    fn weekdays_at_9() {
        assert_eq!(cal("0 9 * * 1-5"), "Mon..Fri *-*-* 09:00:00");
    }

    #[test]
    fn first_of_month_midnight() {
        assert_eq!(cal("0 0 1 * *"), "*-*-01 00:00:00");
    }

    #[test]
    fn specific_months() {
        assert_eq!(cal("0 0 1 1,6 *"), "*-01,06-01 00:00:00");
    }

    #[test]
    fn range_with_step() {
        assert_eq!(cal("0-30/10 * * * *"), "*-*-* *:00,10,20,30:00");
    }

    #[test]
    fn special_daily() {
        assert_eq!(cal("@daily"), "*-*-* 00:00:00");
    }

    #[test]
    fn special_hourly() {
        assert_eq!(cal("@hourly"), "*-*-* *:00:00");
    }

    #[test]
    fn special_weekly() {
        assert_eq!(cal("@weekly"), "Mon *-*-* 00:00:00");
    }

    #[test]
    fn special_monthly() {
        assert_eq!(cal("@monthly"), "*-*-01 00:00:00");
    }

    #[test]
    fn special_yearly() {
        assert_eq!(cal("@yearly"), "*-01-01 00:00:00");
    }

    #[test]
    fn special_reboot() {
        let result = parse("@reboot").unwrap();
        assert!(result.on_calendar.is_none());
        assert_eq!(result.on_boot_sec.unwrap(), "1min");
    }

    #[test]
    fn special_service() {
        let result = parse("@service").unwrap();
        assert!(result.on_calendar.is_none());
        assert!(result.on_boot_sec.is_none());
        assert!(result.is_service);
        assert_eq!(result.display.unwrap(), "@service");
    }

    #[test]
    fn sunday_both_forms() {
        // cron: 0 and 7 both mean Sunday
        assert_eq!(cal("0 9 * * 0"), "Sun *-*-* 09:00:00");
        assert_eq!(cal("0 9 * * 7"), "Sun *-*-* 09:00:00");
    }

    #[test]
    fn multiple_dow() {
        assert_eq!(cal("0 9 * * 1,3,5"), "Mon,Wed,Fri *-*-* 09:00:00");
    }

    #[test]
    fn invalid_field_count() {
        assert!(parse("* * *").is_err());
    }

    // Extended syntax tests
    #[test]
    fn extended_daily_hour() {
        assert_eq!(cal("@daily/9"), "*-*-* 09:00:00");
        assert_eq!(display("@daily/9"), "@daily/9");
    }

    #[test]
    fn extended_daily_hour_minute() {
        assert_eq!(cal("@daily/9:30"), "*-*-* 09:30:00");
        assert_eq!(display("@daily/9:30"), "@daily/9:30");
    }

    #[test]
    fn extended_daily_zero_minute() {
        assert_eq!(cal("@daily/9:00"), "*-*-* 09:00:00");
        assert_eq!(display("@daily/9:00"), "@daily/9"); // normalized to no :00
    }

    #[test]
    fn extended_weekday_full() {
        assert_eq!(cal("@monday/9"), "Mon *-*-* 09:00:00");
        assert_eq!(display("@monday/9"), "@mon/9");
    }

    #[test]
    fn extended_weekday_short() {
        assert_eq!(cal("@mon/9:30"), "Mon *-*-* 09:30:00");
        assert_eq!(display("@mon/9:30"), "@mon/9:30");
    }

    #[test]
    fn extended_weekday_all_days() {
        assert_eq!(cal("@sunday/10"), "Sun *-*-* 10:00:00");
        assert_eq!(cal("@tuesday/10"), "Tue *-*-* 10:00:00");
        assert_eq!(cal("@wednesday/10"), "Wed *-*-* 10:00:00");
        assert_eq!(cal("@thursday/10"), "Thu *-*-* 10:00:00");
        assert_eq!(cal("@friday/10"), "Fri *-*-* 10:00:00");
        assert_eq!(cal("@saturday/10"), "Sat *-*-* 10:00:00");
    }

    #[test]
    fn extended_weekly_alt_syntax() {
        assert_eq!(cal("@weekly/mon/9"), "Mon *-*-* 09:00:00");
        assert_eq!(display("@weekly/mon/9"), "@mon/9");
    }

    #[test]
    fn extended_ordinal_1st() {
        assert_eq!(cal("@1st/8"), "*-*-01 08:00:00");
        assert_eq!(display("@1st/8"), "@1st/8");
    }

    #[test]
    fn extended_ordinal_20th() {
        assert_eq!(cal("@20th/8"), "*-*-20 08:00:00");
        assert_eq!(display("@20th/8"), "@20th/8");
    }

    #[test]
    fn extended_ordinal_22nd() {
        assert_eq!(cal("@22nd/11:30"), "*-*-22 11:30:00");
        assert_eq!(display("@22nd/11:30"), "@22nd/11:30");
    }

    #[test]
    fn extended_ordinal_3rd() {
        assert_eq!(cal("@3rd/12"), "*-*-03 12:00:00");
        assert_eq!(display("@3rd/12"), "@3rd/12");
    }

    #[test]
    fn extended_monthly_alt_syntax() {
        assert_eq!(cal("@monthly/1/9"), "*-*-01 09:00:00");
        assert_eq!(display("@monthly/1/9"), "@1st/9");
    }

    #[test]
    fn extended_monthly_alt_with_minute() {
        assert_eq!(cal("@monthly/26/11:30"), "*-*-26 11:30:00");
        assert_eq!(display("@monthly/26/11:30"), "@26th/11:30");
    }

    // Error case tests
    #[test]
    fn error_weekday_missing_time() {
        let err = parse("@monday").unwrap_err();
        assert!(err.to_string().contains("Missing time"));
    }

    #[test]
    fn error_ordinal_missing_time() {
        let err = parse("@1st").unwrap_err();
        assert!(err.to_string().contains("Missing time"));
    }

    #[test]
    fn error_day_out_of_range_32() {
        let err = parse("@32nd/9").unwrap_err();
        assert!(err.to_string().contains("Day must be 1-31"));
    }

    #[test]
    fn error_day_out_of_range_0() {
        let err = parse("@0th/9").unwrap_err();
        assert!(err.to_string().contains("Day must be 1-31"));
    }

    #[test]
    fn error_hour_out_of_range() {
        let err = parse("@daily/25").unwrap_err();
        assert!(err.to_string().contains("Hour must be 0-23"));
    }

    #[test]
    fn error_minute_out_of_range() {
        let err = parse("@daily/9:70").unwrap_err();
        assert!(err.to_string().contains("Minute must be 0-59"));
    }

    #[test]
    fn error_invalid_hour_string() {
        let err = parse("@daily/abc").unwrap_err();
        assert!(err.to_string().contains("Invalid hour"));
    }
}
