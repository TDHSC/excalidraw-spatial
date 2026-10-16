pub mod ai_report;
pub mod ascii_map;

use crate::linter::{LintIssue, Severity};

pub fn format_lint_text(issues: &[LintIssue]) -> String {
    let mut out = String::new();
    let summary = count_severities(issues);
    for issue in issues {
        let sev = match issue.severity {
            Severity::Error => "ERROR",
            Severity::Warn => "WARN ",
            Severity::Info => "INFO ",
        };
        out.push_str(sev);
        out.push(' ');
        out.push_str(issue.rule);
        out.push(' ');
        out.push_str(&issue.message);
        out.push('\n');
        if let Some(fix) = &issue.fix {
            out.push_str("      fix: ");
            out.push_str(fix);
            out.push('\n');
        }
        if let Some(command) = &issue.command {
            out.push_str("      run: ");
            out.push_str(command);
            out.push('\n');
        }
    }
    out.push_str(&format!(
        "errors: {}, warnings: {}, info: {}\n",
        summary.errors, summary.warnings, summary.info
    ));
    out
}

pub fn format_lint_json(file: &str, issues: &[LintIssue]) -> String {
    let report = LintJson {
        file,
        summary: count_severities(issues),
        issues,
    };
    let mut s = serde_json::to_string_pretty(&report).expect("lint json");
    s.push('\n');
    s
}

#[derive(serde::Serialize)]
struct LintJson<'a> {
    file: &'a str,
    summary: LintSummary,
    issues: &'a [LintIssue],
}

#[derive(serde::Serialize)]
struct LintSummary {
    errors: usize,
    warnings: usize,
    info: usize,
}

fn count_severities(issues: &[LintIssue]) -> LintSummary {
    let mut summary = LintSummary {
        errors: 0,
        warnings: 0,
        info: 0,
    };
    for issue in issues {
        match issue.severity {
            Severity::Error => summary.errors += 1,
            Severity::Warn => summary.warnings += 1,
            Severity::Info => summary.info += 1,
        }
    }
    summary
}
