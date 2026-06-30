use anstyle::AnsiColor;

pub fn print_warnings(warnings: &[String]) {
    for warning in warnings {
        print_warning(warning);
    }

    if let Some(summary) = warning_summary(warnings.len()) {
        print_warning(&summary);
    }
}

pub fn print_warning(message: &dyn std::fmt::Display) {
    print_diagnostic(AnsiColor::Yellow, "WARNING", message);
}

pub fn print_error(error: &dyn std::fmt::Display) {
    print_diagnostic(AnsiColor::Red, "ERROR", error);
}

fn print_diagnostic(color: AnsiColor, label: &str, message: &dyn std::fmt::Display) {
    let style = color.on_default().bold();
    anstream::eprintln!("{style}{label}{style:#}: {message}");
}

fn warning_summary(count: usize) -> Option<String> {
    if count > 1 {
        Some(format!("{count} warnings emitted"))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::warning_summary;

    #[test]
    fn omits_warning_summary_for_zero_or_one_warning() {
        assert_eq!(warning_summary(0), None);
        assert_eq!(warning_summary(1), None);
    }

    #[test]
    fn summarizes_multiple_warnings() {
        assert_eq!(warning_summary(3), Some("3 warnings emitted".to_string()));
    }
}
