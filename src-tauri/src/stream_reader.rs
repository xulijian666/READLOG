use chrono::Datelike;

use crate::filter::{event_matches, should_prune_after_end, QueryFilter};
use crate::parser::{is_timestamp_line, LogEvent, LogEventBuilder, ServerContext};

pub struct LogStreamReader {
    buffer: String,
    current_event: Option<LogEventBuilder>,
    filter: QueryFilter,
    server: ServerContext,
    result_queue: Vec<LogEvent>,
    batch_size: usize,
    current_year: i32,
    next_line_offset: usize,
    scanned_events_delta: usize,
    pruned: bool,
}

impl LogStreamReader {
    pub fn new(server: ServerContext, filter: QueryFilter, batch_size: usize, current_year: i32) -> Self {
        Self {
            buffer: String::new(),
            current_event: None,
            filter,
            server,
            result_queue: Vec::new(),
            batch_size: batch_size.max(1),
            current_year,
            next_line_offset: 1,
            scanned_events_delta: 0,
            pruned: false,
        }
    }

    pub fn with_current_year(server: ServerContext, filter: QueryFilter, batch_size: usize) -> Self {
        Self::new(server, filter, batch_size, chrono::Utc::now().year())
    }

    pub fn process_chunk(&mut self, chunk: &[u8]) -> Vec<Vec<LogEvent>> {
        if self.pruned {
            return Vec::new();
        }

        self.buffer.push_str(&String::from_utf8_lossy(chunk));
        let mut batches = Vec::new();

        while let Some(newline_index) = self.buffer.find('\n') {
            let mut line = self.buffer[..newline_index].to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            self.buffer.drain(..=newline_index);
            self.process_line(&line, &mut batches);
            if self.pruned {
                break;
            }
        }

        batches
    }

    pub fn finish(&mut self) -> Vec<Vec<LogEvent>> {
        let mut batches = Vec::new();
        if !self.buffer.is_empty() && !self.pruned {
            let line = std::mem::take(&mut self.buffer);
            self.process_line(line.trim_end_matches('\r'), &mut batches);
        }
        self.finalize_current(&mut batches);
        if !self.result_queue.is_empty() {
            batches.push(std::mem::take(&mut self.result_queue));
        }
        batches
    }

    pub fn scanned_bytes(&self) -> usize {
        0
    }

    pub fn is_pruned(&self) -> bool {
        self.pruned
    }

    pub fn take_scanned_events_delta(&mut self) -> usize {
        std::mem::take(&mut self.scanned_events_delta)
    }

    pub fn drain_pending_results(&mut self) -> Option<Vec<LogEvent>> {
        if self.result_queue.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.result_queue))
        }
    }

    fn process_line(&mut self, line: &str, batches: &mut Vec<Vec<LogEvent>>) {
        if is_timestamp_line(line) {
            self.finalize_current(batches);
            if let Some(builder) =
                LogEventBuilder::from_first_line(&self.server, self.next_line_offset, line, self.current_year)
            {
                if should_prune_after_end(builder.clone().build().timestamp, &self.filter) {
                    self.pruned = true;
                    return;
                }
                self.current_event = Some(builder);
            }
        } else if let Some(builder) = self.current_event.as_mut() {
            builder.append_line(line);
        }
        self.next_line_offset += 1;
    }

    fn finalize_current(&mut self, batches: &mut Vec<Vec<LogEvent>>) {
        let Some(builder) = self.current_event.take() else {
            return;
        };
        let event = builder.build();
        self.scanned_events_delta += 1;
        if let Some(event) = event_matches(event, &self.filter) {
            self.result_queue.push(event);
        }
        if self.result_queue.len() >= self.batch_size {
            batches.push(std::mem::take(&mut self.result_queue));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filter::QueryFilter;
    use crate::parser::ServerContext;

    #[test]
    fn emits_batches_as_chunks_are_processed() {
        let server = ServerContext {
            id: "s1".to_string(),
            name: "SIT-124".to_string(),
            display_order: 0,
        };
        let mut reader = LogStreamReader::new(
            server,
            QueryFilter {
                start_time: None,
                end_time: None,
                keyword: String::new(),
                level: "ALL".to_string(),
            },
            2,
            2026,
        );

        let first = reader.process_chunk(
            b"26-04-30 13:00:01 INFO [-exec-1] c.A - one\n26-04-30 13:00:02 WARN [-exec-2]",
        );
        assert!(first.is_empty());

        let second = reader.process_chunk(
            b" c.A - two\n26-04-30 13:00:03 ERROR [-exec-3] c.A - three\n",
        );

        assert_eq!(second.len(), 1);
        assert_eq!(second[0].len(), 2);
        let tail = reader.finish();
        assert_eq!(tail.len(), 1);
        assert_eq!(tail[0].len(), 1);
    }

    #[test]
    fn can_flush_partial_matches_before_batch_is_full() {
        let server = ServerContext {
            id: "s1".to_string(),
            name: "SIT-124".to_string(),
            display_order: 0,
        };
        let mut reader = LogStreamReader::new(
            server,
            QueryFilter {
                start_time: None,
                end_time: None,
                keyword: "needle".to_string(),
                level: "ALL".to_string(),
            },
            500,
            2026,
        );

        let batches = reader.process_chunk(
            b"26-04-30 13:00:01 INFO [-exec-1] c.A - needle\n26-04-30 13:00:02 INFO [-exec-1] c.A - next\n",
        );

        assert!(batches.is_empty());
        let partial = reader.drain_pending_results().expect("partial match should be available");
        assert_eq!(partial.len(), 1);
    }
}
