use crate::traits::{CalendarAttendee, CalendarMeetingEvent};
use anyhow::Result;
use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Utc};

/// Fetches and parses a remote RFC 5545 `.ics` calendar URL (Google Calendar, Outlook, Apple iCloud).
pub async fn fetch_and_parse_ics_url(url: &str) -> Result<Vec<CalendarMeetingEvent>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let body = client.get(url).send().await?.error_for_status()?.text().await?;
    Ok(parse_ics_events(&body, Utc::now()))
}

/// Parses RFC 5545 `VEVENT` blocks into `CalendarMeetingEvent` items within `[-1h, +48h]`.
pub fn parse_ics_events(raw_ics: &str, now: DateTime<Utc>) -> Vec<CalendarMeetingEvent> {
    // Unfold RFC 5545 continuation lines (CRLF + space/tab)
    let unfolded = raw_ics
        .replace("\r\n ", "")
        .replace("\r\n\t", "")
        .replace("\n ", "")
        .replace("\n\t", "");

    let mut events = Vec::new();
    let mut in_event = false;
    let mut uid = String::new();
    let mut summary = String::new();
    let mut dtstart: Option<DateTime<Utc>> = None;
    let mut dtend: Option<DateTime<Utc>> = None;
    let mut location = String::new();
    let mut description = String::new();
    let mut attendees: Vec<CalendarAttendee> = Vec::new();

    for line in unfolded.lines() {
        let trimmed = line.trim();
        if trimmed == "BEGIN:VEVENT" {
            in_event = true;
            uid.clear();
            summary.clear();
            dtstart = None;
            dtend = None;
            location.clear();
            description.clear();
            attendees.clear();
            continue;
        }
        if trimmed == "END:VEVENT" && in_event {
            in_event = false;
            if let Some(start) = dtstart {
                let end = dtend.unwrap_or_else(|| start + Duration::minutes(45));
                if end >= now - Duration::hours(1) && start <= now + Duration::hours(48) {
                    let conf_url = extract_conference_link(&format!("{location} {description}"));
                    events.push(CalendarMeetingEvent {
                        event_id: if uid.is_empty() {
                            format!("ics_{}_{}", summary, start.timestamp())
                        } else {
                            uid.clone()
                        },
                        title: if summary.is_empty() {
                            "Kalender-Meeting".into()
                        } else {
                            summary.replace("\\,", ",").replace("\\n", " ")
                        },
                        starts_at: start,
                        ends_at: end,
                        conference_url: conf_url,
                        attendees: attendees.clone(),
                    });
                }
            }
            continue;
        }
        if !in_event {
            continue;
        }

        if let Some((key_part, value)) = trimmed.split_once(':') {
            let key_upper = key_part.split(';').next().unwrap_or("").to_uppercase();
            match key_upper.as_str() {
                "UID" => uid = value.to_string(),
                "SUMMARY" => summary = value.to_string(),
                "LOCATION" => location = value.to_string(),
                "DESCRIPTION" => description = value.to_string(),
                "DTSTART" => dtstart = parse_ics_datetime(value),
                "DTEND" => dtend = parse_ics_datetime(value),
                "ATTENDEE" => {
                    let email = value
                        .trim_start_matches("mailto:")
                        .trim_start_matches("MAILTO:")
                        .trim()
                        .to_string();
                    let cn = key_part
                        .split(';')
                        .find_map(|p| {
                            p.strip_prefix("CN=")
                                .or_else(|| p.strip_prefix("cn="))
                                .map(|s| s.trim_matches('"').to_string())
                        })
                        .unwrap_or_else(|| email.clone());
                    if !email.is_empty() {
                        attendees.push(CalendarAttendee { name: cn, email });
                    }
                }
                _ => {}
            }
        }
    }

    events.sort_by_key(|e| e.starts_at);
    events
}

fn parse_ics_datetime(val: &str) -> Option<DateTime<Utc>> {
    let clean = val.trim();
    if let Ok(dt) = DateTime::parse_from_rfc3339(clean) {
        return Some(dt.with_timezone(&Utc));
    }
    if clean.ends_with('Z') && clean.len() >= 16 {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&clean[..15], "%Y%m%dT%H%M%S") {
            return Some(Utc.from_utc_datetime(&naive));
        }
    }
    if clean.len() >= 15 {
        if let Ok(naive) = NaiveDateTime::parse_from_str(&clean[..15], "%Y%m%dT%H%M%S") {
            return Some(Utc.from_utc_datetime(&naive));
        }
    }
    None
}

fn extract_conference_link(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c| c == '<' || c == '>' || c == '"' || c == ')');
        if cleaned.contains("meet.google.com/")
            || cleaned.contains("zoom.us/j/")
            || cleaned.contains("teams.microsoft.com/l/meetup-join")
        {
            return Some(cleaned.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ics_vevents_attendees_and_meet_urls() {
        let now = Utc::now();
        let start_str = (now + Duration::minutes(20)).format("%Y%m%dT%H%M%SZ");
        let end_str = (now + Duration::minutes(50)).format("%Y%m%dT%H%M%SZ");
        let sample = format!(
            "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nUID:evt-123\r\nSUMMARY:M5 Pro Design Review\r\nDTSTART:{start_str}\r\nDTEND:{end_str}\r\nLOCATION:https://meet.google.com/abc-defg-hij\r\nATTENDEE;CN=\"Anna Weber\":mailto:anna@nordlicht.io\r\nEND:VEVENT\r\nEND:VCALENDAR"
        );
        let events = parse_ics_events(&sample, now);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "M5 Pro Design Review");
        assert_eq!(
            events[0].conference_url.as_deref(),
            Some("https://meet.google.com/abc-defg-hij")
        );
        assert_eq!(events[0].attendees[0].name, "Anna Weber");
    }
}
