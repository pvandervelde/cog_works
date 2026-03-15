//! Tests for the labels module.

use super::*;

mod round_trip_tests {
    use super::*;

    #[test]
    fn round_trip_all_variants() {
        let all = [
            PipelineLabel::Run,
            PipelineLabel::Restart,
            PipelineLabel::Running,
            PipelineLabel::Done,
            PipelineLabel::Failed,
            PipelineLabel::Escalated,
            PipelineLabel::HumanGatePending,
            PipelineLabel::HumanGateApproved,
            PipelineLabel::HumanGateRejected,
            PipelineLabel::Lock,
            PipelineLabel::SecurityHold,
            PipelineLabel::SafetyCritical,
        ];
        for label in all {
            let s = generate_label(&label);
            assert_eq!(
                parse_label(&s),
                Some(label),
                "round-trip failed for {label:?}: generated '{s}' but parse returned None"
            );
        }
    }
}

mod parse_tests {
    use super::*;

    #[test]
    fn unknown_label_returns_none() {
        assert_eq!(parse_label("bug"), None);
        assert_eq!(parse_label("cogworks:unknown"), None);
        assert_eq!(parse_label(""), None);
    }
}
