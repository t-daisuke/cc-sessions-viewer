use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub dir_name: String,
    pub original_path: String,
    pub session_count: usize,
}

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub project_name: String,
    pub preview: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub message_count: usize,
    pub git_branch: String,
    pub summary: String,
}

impl SessionInfo {
    pub fn timestamp_str(&self) -> String {
        self.timestamp
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    ToolUse,
    ToolResult,
    Progress,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: MessageRole,
    pub text: String,
    pub timestamp: Option<DateTime<Utc>>,
    pub tool_name: Option<String>,
}

impl Message {
    pub fn timestamp_str(&self) -> String {
        self.timestamp
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    }

    pub fn role_label(&self) -> &'static str {
        match self.role {
            MessageRole::User => "USER",
            MessageRole::Assistant => "ASSISTANT",
            MessageRole::System => "SYSTEM",
            MessageRole::ToolUse => "TOOL",
            MessageRole::ToolResult => "RESULT",
            MessageRole::Progress => "PROGRESS",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub session_id: String,
    pub project_path: String,
    pub dir_name: String,
    pub git_branch: String,
    pub created_at: String,
    pub prompts: Vec<String>,
    pub best_match_prompt: String,
    pub best_match_indices: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFilter {
    Yesterday,
    Week,
    Month,
    All,
}

impl TimeFilter {
    pub fn label(&self) -> &'static str {
        match self {
            TimeFilter::Yesterday => "Yesterday",
            TimeFilter::Week => "Week",
            TimeFilter::Month => "Month",
            TimeFilter::All => "All",
        }
    }

    pub fn all_filters() -> &'static [TimeFilter] {
        &[
            TimeFilter::Yesterday,
            TimeFilter::Week,
            TimeFilter::Month,
            TimeFilter::All,
        ]
    }

    pub fn next(&self) -> TimeFilter {
        match self {
            TimeFilter::Yesterday => TimeFilter::Week,
            TimeFilter::Week => TimeFilter::Month,
            TimeFilter::Month => TimeFilter::All,
            TimeFilter::All => TimeFilter::Yesterday,
        }
    }

    pub fn prev(&self) -> TimeFilter {
        match self {
            TimeFilter::Yesterday => TimeFilter::All,
            TimeFilter::Week => TimeFilter::Yesterday,
            TimeFilter::Month => TimeFilter::Week,
            TimeFilter::All => TimeFilter::Month,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // ============================================================
    // TimeFilter tests
    // ============================================================

    #[test]
    fn time_filter_next_cycle() {
        // Yesterday -> Week -> Month -> All -> Yesterday (1周)
        let start = TimeFilter::Yesterday;
        let step1 = start.next();
        assert_eq!(step1, TimeFilter::Week);
        let step2 = step1.next();
        assert_eq!(step2, TimeFilter::Month);
        let step3 = step2.next();
        assert_eq!(step3, TimeFilter::All);
        let step4 = step3.next();
        assert_eq!(step4, TimeFilter::Yesterday);
    }

    #[test]
    fn time_filter_prev_cycle() {
        // All -> Month -> Week -> Yesterday -> All (1周)
        let start = TimeFilter::All;
        let step1 = start.prev();
        assert_eq!(step1, TimeFilter::Month);
        let step2 = step1.prev();
        assert_eq!(step2, TimeFilter::Week);
        let step3 = step2.prev();
        assert_eq!(step3, TimeFilter::Yesterday);
        let step4 = step3.prev();
        assert_eq!(step4, TimeFilter::All);
    }

    #[test]
    fn time_filter_next_prev_inverse() {
        // 任意のフィルタに対して filter.next().prev() == filter
        for &filter in TimeFilter::all_filters() {
            assert_eq!(filter.next().prev(), filter);
            assert_eq!(filter.prev().next(), filter);
        }
    }

    #[test]
    fn time_filter_label() {
        assert_eq!(TimeFilter::Yesterday.label(), "Yesterday");
        assert_eq!(TimeFilter::Week.label(), "Week");
        assert_eq!(TimeFilter::Month.label(), "Month");
        assert_eq!(TimeFilter::All.label(), "All");
    }

    #[test]
    fn time_filter_all_filters_length() {
        assert_eq!(TimeFilter::all_filters().len(), 4);
    }

    #[test]
    fn time_filter_all_filters_contains_all_variants() {
        let filters = TimeFilter::all_filters();
        assert!(filters.contains(&TimeFilter::Yesterday));
        assert!(filters.contains(&TimeFilter::Week));
        assert!(filters.contains(&TimeFilter::Month));
        assert!(filters.contains(&TimeFilter::All));
    }

    // ============================================================
    // Message tests
    // ============================================================

    fn make_message(role: MessageRole, timestamp: Option<DateTime<Utc>>) -> Message {
        Message {
            role,
            text: String::new(),
            timestamp,
            tool_name: None,
        }
    }

    #[test]
    fn message_role_label_user() {
        assert_eq!(make_message(MessageRole::User, None).role_label(), "USER");
    }

    #[test]
    fn message_role_label_assistant() {
        assert_eq!(
            make_message(MessageRole::Assistant, None).role_label(),
            "ASSISTANT"
        );
    }

    #[test]
    fn message_role_label_system() {
        assert_eq!(
            make_message(MessageRole::System, None).role_label(),
            "SYSTEM"
        );
    }

    #[test]
    fn message_role_label_tool_use() {
        assert_eq!(
            make_message(MessageRole::ToolUse, None).role_label(),
            "TOOL"
        );
    }

    #[test]
    fn message_role_label_tool_result() {
        assert_eq!(
            make_message(MessageRole::ToolResult, None).role_label(),
            "RESULT"
        );
    }

    #[test]
    fn message_role_label_progress() {
        assert_eq!(
            make_message(MessageRole::Progress, None).role_label(),
            "PROGRESS"
        );
    }

    #[test]
    fn message_timestamp_str_none() {
        let msg = make_message(MessageRole::User, None);
        assert_eq!(msg.timestamp_str(), "");
    }

    #[test]
    fn message_timestamp_str_some() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let msg = make_message(MessageRole::User, Some(dt));
        assert_eq!(msg.timestamp_str(), "2024-01-15 10:30:00");
    }

    // ============================================================
    // SessionInfo tests
    // ============================================================

    fn make_session(timestamp: Option<DateTime<Utc>>) -> SessionInfo {
        SessionInfo {
            session_id: String::new(),
            project_name: String::new(),
            preview: String::new(),
            timestamp,
            message_count: 0,
            git_branch: String::new(),
            summary: String::new(),
        }
    }

    #[test]
    fn session_info_timestamp_str_none() {
        let session = make_session(None);
        assert_eq!(session.timestamp_str(), "");
    }

    #[test]
    fn session_info_timestamp_str_some() {
        let dt = Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap();
        let session = make_session(Some(dt));
        assert_eq!(session.timestamp_str(), "2024-01-15 10:30:00");
    }
}
