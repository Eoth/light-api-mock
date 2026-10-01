use super::*;

#[test]
fn log_filter_defaults_to_informational_messages() {
    assert_eq!(log_filter(None), ("mimicway=info".to_string(), false));
    assert_eq!(log_filter(Some("  ")), ("mimicway=info".to_string(), false));
}

#[test]
fn log_filter_keeps_any_other_value() {
    assert_eq!(
        log_filter(Some("mimicway=debug,tower_http=info")),
        ("mimicway=debug,tower_http=info".to_string(), false)
    );
}

#[test]
fn log_filter_reads_the_former_name_as_the_new_one() {
    assert_eq!(
        log_filter(Some("light_mock=debug")),
        ("mimicway=debug".to_string(), true)
    );
    assert_eq!(
        log_filter(Some("warn,light_mock::script=trace")),
        ("warn,mimicway::script=trace".to_string(), true)
    );
}
