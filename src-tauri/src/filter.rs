use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::parser::{HighlightRange, LogEvent};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilter {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub keyword: String,
    pub level: String,
}

pub fn event_matches(mut event: LogEvent, filter: &QueryFilter) -> Option<LogEvent> {
    if let Some(start_time) = filter.start_time {
        if event.timestamp < start_time {
            return None;
        }
    }

    if let Some(end_time) = filter.end_time {
        if event.timestamp > end_time {
            return None;
        }
    }

    if filter.level != "ALL" && !event.level.eq_ignore_ascii_case(&filter.level) {
        return None;
    }

    let keyword = filter.keyword.trim();
    if keyword.is_empty() {
        return Some(event);
    }

    let ranges = find_keyword_ranges(&event.raw_text, keyword);
    if ranges.is_empty() {
        return None;
    }

    event.highlighted = true;
    event.highlight_ranges = ranges;
    Some(event)
}

pub fn should_prune_after_end(timestamp: DateTime<Utc>, filter: &QueryFilter) -> bool {
    filter.end_time.is_some_and(|end_time| timestamp > end_time)
}

fn find_keyword_ranges(text: &str, keyword: &str) -> Vec<HighlightRange> {
    let lower_text = text.to_lowercase();
    let lower_keyword = keyword.to_lowercase();
    if lower_keyword.is_empty() {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut search_start = 0;
    while let Some(relative) = lower_text[search_start..].find(&lower_keyword) {
        let start = search_start + relative;
        let end = start + lower_keyword.len();
        ranges.push(HighlightRange { start, end });
        search_start = end;
    }
    ranges
}
