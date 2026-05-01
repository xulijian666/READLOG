use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HighlightRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub id: String,
    pub server_id: String,
    pub server_name: String,
    pub server_display_order: usize,
    pub line_offset: usize,
    pub timestamp: DateTime<Utc>,
    pub level: String,
    pub thread: String,
    pub class_name: String,
    pub first_line_content: String,
    pub raw_text: String,
    pub line_count: usize,
    pub highlighted: bool,
    pub highlight_ranges: Vec<HighlightRange>,
}

#[derive(Clone, Debug)]
pub struct ServerContext {
    pub id: String,
    pub name: String,
    pub display_order: usize,
}

#[derive(Clone, Debug)]
pub struct LogEventBuilder {
    server: ServerContext,
    line_offset: usize,
    timestamp: DateTime<Utc>,
    level: String,
    thread: String,
    class_name: String,
    first_line_content: String,
    lines: Vec<String>,
}

impl LogEventBuilder {
    pub fn from_first_line(
        server: &ServerContext,
        line_offset: usize,
        line: &str,
        current_year: i32,
    ) -> Option<Self> {
        let timestamp = parse_timestamp(line, current_year)?;
        let metadata = parse_line_metadata(line);
        Some(Self {
            server: server.clone(),
            line_offset,
            timestamp,
            level: metadata.level,
            thread: metadata.thread,
            class_name: metadata.class_name,
            first_line_content: line.to_string(),
            lines: vec![line.to_string()],
        })
    }

    pub fn append_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn build(self) -> LogEvent {
        let raw_text = self.lines.join("\n");
        LogEvent {
            id: format!("{}:{}", self.server.id, self.line_offset),
            server_id: self.server.id,
            server_name: self.server.name,
            server_display_order: self.server.display_order,
            line_offset: self.line_offset,
            timestamp: self.timestamp,
            level: self.level,
            thread: self.thread,
            class_name: self.class_name,
            first_line_content: self.first_line_content,
            raw_text,
            line_count: self.lines.len(),
            highlighted: false,
            highlight_ranges: Vec::new(),
        }
    }
}

#[derive(Debug)]
struct LineMetadata {
    level: String,
    thread: String,
    class_name: String,
}

pub fn parse_log_events(text: &str, server: &ServerContext, current_year: i32) -> Vec<LogEvent> {
    let mut events = Vec::new();
    let mut current: Option<LogEventBuilder> = None;

    for (index, line) in text.lines().enumerate() {
        let line_offset = index + 1;
        if is_timestamp_line(line) {
            if let Some(builder) = current.take() {
                events.push(builder.build());
            }
            current = LogEventBuilder::from_first_line(server, line_offset, line, current_year);
        } else if let Some(builder) = current.as_mut() {
            builder.append_line(line);
        }
    }

    if let Some(builder) = current {
        events.push(builder.build());
    }

    events
}

pub fn is_timestamp_line(line: &str) -> bool {
    parse_timestamp(line, Utc::now().year()).is_some()
}

pub fn parse_timestamp(line: &str, current_year: i32) -> Option<DateTime<Utc>> {
    let stamp = line.get(0..17)?;
    let mut parts = stamp.split(['-', ' ', ':']);
    let yy: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let hour: u32 = parts.next()?.parse().ok()?;
    let minute: u32 = parts.next()?.parse().ok()?;
    let second: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    let current_century = (current_year / 100) * 100;
    let current_two_digit = current_year % 100;
    let year = if yy > current_two_digit {
        current_century - 100 + yy
    } else {
        current_century + yy
    };

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let time = NaiveTime::from_hms_opt(hour, minute, second)?;
    Some(Utc.from_utc_datetime(&NaiveDateTime::new(date, time)))
}

fn parse_line_metadata(line: &str) -> LineMetadata {
    let rest = line.get(17..).unwrap_or("").trim_start_matches(|c: char| c == ',' || c.is_ascii_digit()).trim();
    let mut tokens = rest.split_whitespace();
    let level = tokens.next().unwrap_or("INFO").to_string();

    let thread = rest
        .split_once('[')
        .and_then(|(_, tail)| tail.split_once(']'))
        .map(|(value, _)| value.to_string())
        .unwrap_or_default();

    let after_thread = rest
        .split_once(']')
        .map(|(_, tail)| tail.trim())
        .unwrap_or(rest);
    let class_name = after_thread
        .split_once(" - ")
        .map(|(class, _)| class.trim().to_string())
        .unwrap_or_default();

    LineMetadata {
        level,
        thread,
        class_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::{event_matches, should_prune_after_end, QueryFilter};
    use chrono::{Datelike, NaiveDate};

    fn server() -> ServerContext {
        ServerContext {
            id: "s1".to_string(),
            name: "SIT-124".to_string(),
            display_order: 0,
        }
    }

    #[test]
    fn parses_multiline_events_without_losing_continuation_lines() {
        let text = concat!(
            "26-04-30 13:00:02,465 DEBUG [-exec-36] com.demo.Controller - start\n",
            "{ \"sql\": \"select *\",\n",
            "  \"status\": \"ok\" }\n",
            "26-04-30 13:00:03 INFO [-exec-40] com.demo.Redis - done\n"
        );

        let events = parse_log_events(text, &server(), 2026);

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].line_offset, 1);
        assert_eq!(events[0].line_count, 3);
        assert!(events[0].raw_text.contains("\"status\": \"ok\""));
        assert_eq!(events[1].level, "INFO");
    }

    #[test]
    fn infers_century_for_two_digit_years() {
        let ts_26 = parse_timestamp("26-04-30 13:00:02,465", 2026).unwrap();
        let ts_99 = parse_timestamp("99-12-31 23:59:59", 2026).unwrap();

        assert_eq!(ts_26.year(), 2026);
        assert_eq!(ts_99.year(), 1999);
        assert_eq!(ts_26.date_naive(), NaiveDate::from_ymd_opt(2026, 4, 30).unwrap());
    }

    #[test]
    fn keyword_filter_searches_full_raw_event_and_marks_ranges() {
        let text = concat!(
            "26-04-30 13:00:02 DEBUG [-exec-36] com.demo.Controller - start\n",
            "payload has TraceId-7788\n"
        );
        let event = parse_log_events(text, &server(), 2026).remove(0);
        let filter = QueryFilter {
            start_time: None,
            end_time: None,
            keyword: "TraceId-7788".to_string(),
            level: "ALL".to_string(),
        };

        let matched = event_matches(event, &filter).unwrap();

        assert!(matched.highlighted);
        assert_eq!(matched.highlight_ranges.len(), 1);
    }

    #[test]
    fn filters_by_time_level_and_prunes_after_end_time() {
        let event = parse_log_events(
            "26-04-30 13:00:02 ERROR [-exec-36] com.demo.Controller - failed\n",
            &server(),
            2026,
        )
        .remove(0);
        let filter = QueryFilter {
            start_time: parse_timestamp("26-04-30 13:00:00", 2026),
            end_time: parse_timestamp("26-04-30 13:01:00", 2026),
            keyword: String::new(),
            level: "ERROR".to_string(),
        };

        assert!(event_matches(event.clone(), &filter).is_some());

        let warn_filter = QueryFilter { level: "WARN".to_string(), ..filter.clone() };
        assert!(event_matches(event.clone(), &warn_filter).is_none());

        let early_end = QueryFilter {
            end_time: parse_timestamp("26-04-30 13:00:01", 2026),
            ..filter
        };
        assert!(should_prune_after_end(event.timestamp, &early_end));
    }
}
