use anstyle::AnsiColor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WarningKind {
    SlideOverflow,
    ListNestingClamped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub kind: WarningKind,
    pub slide_number: Option<usize>,
    message: String,
}

impl Warning {
    pub fn new(kind: WarningKind, slide_number: Option<usize>, message: impl Into<String>) -> Self {
        Self {
            kind,
            slide_number,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(slide_number) = self.slide_number {
            write!(f, "slide {slide_number}: {}", self.message)
        } else {
            f.write_str(&self.message)
        }
    }
}

pub fn print_warnings(warnings: &[Warning]) {
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
    use super::{Warning, WarningKind, warning_summary};

    #[test]
    fn formats_warning_with_slide_context() {
        let warning = Warning::new(WarningKind::SlideOverflow, Some(3), "content overflow");

        assert_eq!(warning.to_string(), "slide 3: content overflow");
    }

    #[test]
    fn formats_warning_without_slide_context() {
        let warning = Warning::new(WarningKind::ListNestingClamped, None, "list clamped");

        assert_eq!(warning.to_string(), "list clamped");
    }

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
