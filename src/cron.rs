use anyhow::{bail, Result};

const DOW_NAMES: &[&str] = &["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

pub struct CronSchedule {
    pub on_calendar: Option<String>,
    pub on_boot_sec: Option<String>,
}

pub fn parse(expr: &str) -> Result<CronSchedule> {
    let trimmed = expr.trim();

    if let Some(special) = parse_special(trimmed) {
        return Ok(special);
    }

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
    })
}

fn parse_special(expr: &str) -> Option<CronSchedule> {
    match expr {
        "@yearly" | "@annually" => Some(CronSchedule {
            on_calendar: Some("*-01-01 00:00:00".to_string()),
            on_boot_sec: None,
        }),
        "@monthly" => Some(CronSchedule {
            on_calendar: Some("*-*-01 00:00:00".to_string()),
            on_boot_sec: None,
        }),
        "@weekly" => Some(CronSchedule {
            on_calendar: Some("Mon *-*-* 00:00:00".to_string()),
            on_boot_sec: None,
        }),
        "@daily" | "@midnight" => Some(CronSchedule {
            on_calendar: Some("*-*-* 00:00:00".to_string()),
            on_boot_sec: None,
        }),
        "@hourly" => Some(CronSchedule {
            on_calendar: Some("*-*-* *:00:00".to_string()),
            on_boot_sec: None,
        }),
        "@reboot" => Some(CronSchedule {
            on_calendar: None,
            on_boot_sec: Some("1min".to_string()),
        }),
        _ => None,
    }
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
}
