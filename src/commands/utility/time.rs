use crate::{Context, Error};
use chrono::{FixedOffset, Local, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;
use std::str::FromStr;
use tracing::debug;

enum TimezoneType {
    Named(Tz),
    Offset(FixedOffset),
}

fn parse_timezone(tz_str: &str) -> Result<TimezoneType, Error> {
    let tz_upper = tz_str.to_uppercase();

    if tz_upper.starts_with("GMT") || tz_upper.starts_with("UTC") {
        let offset_str = tz_upper
            .strip_prefix("GMT")
            .or_else(|| tz_upper.strip_prefix("UTC"))
            .unwrap_or("");

        if offset_str.is_empty()
            || offset_str == "+0"
            || offset_str == "+00:00"
            || offset_str == "-0"
        {
            return Ok(TimezoneType::Named(Tz::UTC));
        }

        let offset_str = offset_str.replace(":", "");

        let (sign, rest) = if let Some(rest) = offset_str.strip_prefix('+') {
            (1, rest)
        } else if let Some(rest) = offset_str.strip_prefix('-') {
            (-1, rest)
        } else {
            return Err(format!("Invalid offset format: {}", tz_str).into());
        };

        let hours: i32 = if rest.len() <= 2 {
            rest.parse()
                .map_err(|_| format!("Invalid offset: {}", tz_str))?
        } else {
            rest[..2]
                .parse()
                .map_err(|_| format!("Invalid offset: {}", tz_str))?
        };

        let minutes: i32 = if rest.len() > 2 {
            rest[2..]
                .parse()
                .map_err(|_| format!("Invalid offset: {}", tz_str))?
        } else {
            0
        };

        let total_seconds = sign * (hours * 3600 + minutes * 60);
        let offset = FixedOffset::east_opt(total_seconds)
            .ok_or_else(|| format!("Invalid offset: {}", tz_str))?;

        return Ok(TimezoneType::Offset(offset));
    }

    let tz_str_normalized = match tz_upper.as_str() {
        "EST" => "America/New_York",
        "PST" => "America/Los_Angeles",
        "CST" => "America/Chicago",
        "MST" => "America/Denver",
        "JST" => "Asia/Tokyo",
        "CET" => "Europe/Paris",
        _ => tz_str,
    };

    Tz::from_str(tz_str_normalized)
        .map(TimezoneType::Named)
        .map_err(|_| format!("Invalid timezone: {}", tz_str).into())
}

#[poise::command(slash_command, prefix_command)]
pub async fn time(
    ctx: Context<'_>,
    #[description = "Time (use 'now' or HH:MM format)"] time: String,
    #[description = "Source timezone (optional, defaults to UTC)"] from_tz: Option<String>,
    #[description = "Target timezones (comma-separated, optional)"] to_tz: Option<String>,
) -> Result<(), Error> {
    debug!(
        "time command called with time='{}', from_tz={:?}, to_tz={:?}",
        time, from_tz, to_tz
    );

    let time_trimmed = time.trim();
    let from_tz_str = from_tz.as_deref().unwrap_or("UTC").trim();

    debug!(
        "After trimming: time_trimmed='{}', from_tz_str='{}'",
        time_trimmed, from_tz_str
    );

    let source_tz = parse_timezone(from_tz_str)?;

    let target_timezones: Vec<TimezoneType> = if let Some(tz_list) = to_tz {
        tz_list
            .split(',')
            .map(|s| parse_timezone(s.trim()))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![parse_timezone("UTC")?]
    };

    if time_trimmed.to_lowercase() == "now" {
        let now_utc = Utc::now();

        let (source_time_str, source_date) = match &source_tz {
            TimezoneType::Named(tz) => {
                let source_time = now_utc.with_timezone(tz);
                (
                    source_time.format("%H:%M:%S %Z").to_string(),
                    source_time.date_naive(),
                )
            }
            TimezoneType::Offset(offset) => {
                let source_time = now_utc.with_timezone(offset);
                (
                    source_time.format("%H:%M:%S %:z").to_string(),
                    source_time.date_naive(),
                )
            }
        };

        let mut response = format!(
            "**now in {}:**\n{}\n\n**Conversions:**\n",
            from_tz_str, source_time_str
        );

        for target_tz in target_timezones {
            match target_tz {
                TimezoneType::Named(tz) => {
                    let converted = now_utc.with_timezone(&tz);
                    let converted_date = converted.date_naive();

                    if converted_date == source_date {
                        response.push_str(&format!(
                            "**{}:** {}\n",
                            tz,
                            converted.format("%H:%M:%S")
                        ));
                    } else {
                        response.push_str(&format!(
                            "**{}:** {}\n",
                            tz,
                            converted.format("%Y-%m-%d %H:%M:%S")
                        ));
                    }
                }
                TimezoneType::Offset(offset) => {
                    let converted = now_utc.with_timezone(&offset);
                    let converted_date = converted.date_naive();

                    if converted_date == source_date {
                        response.push_str(&format!(
                            "**GMT{:+}:** {}\n",
                            offset.local_minus_utc() / 3600,
                            converted.format("%H:%M:%S")
                        ));
                    } else {
                        response.push_str(&format!(
                            "**GMT{:+}:** {}\n",
                            offset.local_minus_utc() / 3600,
                            converted.format("%Y-%m-%d %H:%M:%S")
                        ));
                    }
                }
            }
        }

        ctx.say(response).await?;
    } else {
        let naive_time = NaiveTime::parse_from_str(time_trimmed, "%H:%M")
            .or_else(|_| NaiveTime::parse_from_str(time_trimmed, "%H:%M:%S"))
            .map_err(|_| "Invalid time format. Use HH:MM or HH:MM:SS")?;

        let today = Local::now().date_naive();
        let naive_datetime = today.and_time(naive_time);

        let source_datetime_utc = match &source_tz {
            TimezoneType::Named(tz) => tz
                .from_local_datetime(&naive_datetime)
                .single()
                .ok_or("Ambiguous local time")?
                .with_timezone(&Utc),
            TimezoneType::Offset(offset) => offset
                .from_local_datetime(&naive_datetime)
                .single()
                .ok_or("Ambiguous local time")?
                .with_timezone(&Utc),
        };

        let (source_time_str, source_date) = match &source_tz {
            TimezoneType::Named(tz) => {
                let source_time = source_datetime_utc.with_timezone(tz);
                (
                    source_time.format("%H:%M:%S %Z").to_string(),
                    source_time.date_naive(),
                )
            }
            TimezoneType::Offset(offset) => {
                let source_time = source_datetime_utc.with_timezone(offset);
                (
                    source_time.format("%H:%M:%S %:z").to_string(),
                    source_time.date_naive(),
                )
            }
        };

        let mut response = format!(
            "**{} in {}:**\n{}\n\n**Conversions:**\n",
            time_trimmed, from_tz_str, source_time_str
        );

        for target_tz in target_timezones {
            match target_tz {
                TimezoneType::Named(tz) => {
                    let converted = source_datetime_utc.with_timezone(&tz);
                    let converted_date = converted.date_naive();

                    if converted_date == source_date {
                        response.push_str(&format!(
                            "**{}:** {}\n",
                            tz,
                            converted.format("%H:%M:%S")
                        ));
                    } else {
                        response.push_str(&format!(
                            "**{}:** {}\n",
                            tz,
                            converted.format("%Y-%m-%d %H:%M:%S")
                        ));
                    }
                }
                TimezoneType::Offset(offset) => {
                    let converted = source_datetime_utc.with_timezone(&offset);
                    let converted_date = converted.date_naive();

                    if converted_date == source_date {
                        response.push_str(&format!(
                            "**GMT{:+}:** {}\n",
                            offset.local_minus_utc() / 3600,
                            converted.format("%H:%M:%S")
                        ));
                    } else {
                        response.push_str(&format!(
                            "**GMT{:+}:** {}\n",
                            offset.local_minus_utc() / 3600,
                            converted.format("%Y-%m-%d %H:%M:%S")
                        ));
                    }
                }
            }
        }

        ctx.say(response).await?;
    }

    Ok(())
}
