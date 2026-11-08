//! The diagnostics log line format.

use shdc_lib::diag::line;

#[test]
fn a_line_is_the_unix_time_then_the_trimmed_message() {
    assert_eq!(
        line(1794167296, "  main window shown (page loaded)\n"),
        "1794167296 main window shown (page loaded)\n"
    );
}
