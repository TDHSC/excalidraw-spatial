use anyhow::Result;
use clap::Parser;

use crate::schema::Document;
use crate::{Cli, Commands, DrawCommands, LayoutCommands, OutputArgs};

const FILE_PLACEHOLDER: &str = "<batch>";

const DRAW_VERBS: &[&str] = &[
    "node", "connect", "arrow", "route", "reroute", "zone", "frame", "text", "label", "remove",
    "rm", "delete",
];

const LAYOUT_VERBS: &[&str] = &[
    "move",
    "align",
    "distribute",
    "place",
    "fit-container",
    "fit-text",
];

/// Quote-aware tokenizer for one physical batch-script line.
///
/// Leading `'#'` (after ASCII trim) is a comment. `'#'` inside a token is literal.
/// Double-quoted `\"` and `\\` unescape; every other `\X` (including `\n`) is kept
/// as the two characters so `normalize_text` converts `\n` once.
pub fn tokenize_line(line: &str) -> Result<Vec<String>> {
    let line = line.trim_matches(|c: char| c.is_ascii_whitespace());
    if line.is_empty() || line.starts_with('#') {
        return Ok(Vec::new());
    }

    let chars: Vec<char> = line.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        match chars[i] {
            '\'' => {
                i += 1;
                let start = i;
                while i < chars.len() && chars[i] != '\'' {
                    i += 1;
                }
                if i >= chars.len() {
                    anyhow::bail!("unclosed quote");
                }
                tokens.push(chars[start..i].iter().collect());
                i += 1;
            }
            '"' => {
                i += 1;
                let mut token = String::new();
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        match chars[i + 1] {
                            '"' => {
                                token.push('"');
                                i += 2;
                            }
                            '\\' => {
                                token.push('\\');
                                i += 2;
                            }
                            other => {
                                token.push('\\');
                                token.push(other);
                                i += 2;
                            }
                        }
                    } else if chars[i] == '"' {
                        closed = true;
                        i += 1;
                        break;
                    } else {
                        token.push(chars[i]);
                        i += 1;
                    }
                }
                if !closed {
                    anyhow::bail!("unclosed quote");
                }
                tokens.push(token);
            }
            _ => {
                let start = i;
                while i < chars.len() && !chars[i].is_ascii_whitespace() {
                    i += 1;
                }
                tokens.push(chars[start..i].iter().collect());
            }
        }
    }
    Ok(tokens)
}

/// One executable batch line after prefix stripping and clap parsing.
pub enum BatchOp {
    Draw(Box<DrawCommands>),
    Layout(LayoutCommands),
}

impl std::fmt::Debug for BatchOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BatchOp::Draw(_) => f.write_str("BatchOp::Draw(..)"),
            BatchOp::Layout(_) => f.write_str("BatchOp::Layout(..)"),
        }
    }
}

/// Parse one already-tokenized line into a draw or layout command.
///
/// Empty token lists (blank lines and comments) return `Ok(None)`. The
/// synthetic file positional is `"<batch>"`; batch execution never reads it.
pub fn parse_line(line_no: usize, tokens: &[String]) -> Result<Option<BatchOp>> {
    if tokens.is_empty() {
        return Ok(None);
    }
    let mut rest = tokens;
    if rest.first().is_some_and(|t| t == "excal") {
        rest = &rest[1..];
    }
    let verb = rest.first().map(String::as_str).unwrap_or("");
    let (family, subverb, tail) = if verb == "draw" {
        let sub = rest.get(1).map(String::as_str).unwrap_or("");
        if sub == "init" || sub == "batch" {
            anyhow::bail!("line {line_no}: {sub} is not allowed in batch");
        }
        ("draw", sub, rest.get(2..).unwrap_or(&[]))
    } else if verb == "layout" {
        let sub = rest.get(1).map(String::as_str).unwrap_or("");
        ("layout", sub, rest.get(2..).unwrap_or(&[]))
    } else if verb == "init" || verb == "batch" {
        anyhow::bail!("line {line_no}: {verb} is not allowed in batch");
    } else if DRAW_VERBS.contains(&verb) {
        ("draw", verb, rest.get(1..).unwrap_or(&[]))
    } else if LAYOUT_VERBS.contains(&verb) {
        ("layout", verb, rest.get(1..).unwrap_or(&[]))
    } else {
        // Unknown verbs still go through clap so the error text is clap's,
        // with the leading "error: " stripped.
        ("draw", verb, rest.get(1..).unwrap_or(&[]))
    };

    let mut argv: Vec<String> = vec![
        "excal".to_string(),
        family.to_string(),
        subverb.to_string(),
        FILE_PLACEHOLDER.to_string(),
    ];
    argv.extend(tail.iter().cloned());

    let cli = match Cli::try_parse_from(&argv) {
        Ok(cli) => cli,
        Err(e) => {
            let rendered = e.to_string();
            let first = rendered.lines().next().unwrap_or("");
            let msg = first.strip_prefix("error: ").unwrap_or(first);
            anyhow::bail!("line {line_no}: {msg}");
        }
    };

    let op = match cli.command {
        Commands::Draw { command } => {
            reject_output_flags(line_no, draw_output(&command))?;
            BatchOp::Draw(command)
        }
        Commands::Layout { command } => {
            reject_output_flags(line_no, layout_output(&command))?;
            BatchOp::Layout(command)
        }
        Commands::Inspect { .. } | Commands::Lint { .. } => {
            anyhow::bail!("line {line_no}: unsupported batch command");
        }
    };
    Ok(Some(op))
}

fn reject_output_flags(line_no: usize, output: &OutputArgs) -> Result<()> {
    if output.in_place || output.output.is_some() {
        anyhow::bail!("line {line_no}: output flags (--in-place, -o) are not allowed in batch");
    }
    Ok(())
}

fn draw_output(cmd: &DrawCommands) -> &OutputArgs {
    match cmd {
        DrawCommands::Init { output, .. }
        | DrawCommands::Node { output, .. }
        | DrawCommands::Connect { output, .. }
        | DrawCommands::Route { output, .. }
        | DrawCommands::Zone { output, .. }
        | DrawCommands::Frame { output, .. }
        | DrawCommands::Text { output, .. }
        | DrawCommands::Label { output, .. }
        | DrawCommands::Remove { output, .. }
        | DrawCommands::Batch { output, .. } => output,
    }
}

fn layout_output(cmd: &LayoutCommands) -> &OutputArgs {
    match cmd {
        LayoutCommands::Move { output, .. }
        | LayoutCommands::Align { output, .. }
        | LayoutCommands::Distribute { output, .. }
        | LayoutCommands::Place { output, .. }
        | LayoutCommands::FitContainer { output, .. }
        | LayoutCommands::FitText { output, .. } => output,
    }
}

/// Run a full batch script against `doc`, accumulating authoring counts.
///
/// Does not call `doc.finish` or write output. On the first failing line,
/// returns `line {line_no}: {err}` and leaves `doc` as it was after the last
/// successful line.
pub fn execute_script(
    doc: &mut Document,
    script: &str,
    now_ms: i64,
) -> Result<crate::author::AuthorReport> {
    let mut report = crate::author::AuthorReport::default();
    for (idx, line) in script.lines().enumerate() {
        let line_no = idx + 1;
        let tokens = tokenize_line(line).map_err(|err| anyhow::anyhow!("line {line_no}: {err}"))?;
        let Some(op) = parse_line(line_no, &tokens)? else {
            continue;
        };
        doc.refresh();
        match op {
            BatchOp::Draw(cmd) => {
                let line_report = crate::author::apply_draw(doc, &cmd, now_ms)
                    .map_err(|err| anyhow::anyhow!("line {line_no}: {err}"))?;
                report.added += line_report.added;
                report.updated += line_report.updated;
                report.removed += line_report.removed;
            }
            BatchOp::Layout(cmd) => {
                let scene = crate::scene_graph::Scene::build(doc);
                let mut change = crate::mutator::layout_change(&scene, &cmd)
                    .map_err(|err| anyhow::anyhow!("line {line_no}: {err}"))?;
                crate::mutator::strip_nops(&mut change, &scene);
                if !change.is_empty() {
                    let applied = crate::mutator::apply(doc, &scene, &change)
                        .map_err(|err| anyhow::anyhow!("line {line_no}: {err}"))?;
                    doc.refresh();
                    report.updated += applied.moved + applied.resized + applied.connectors;
                }
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, DrawCommands, LayoutCommands, PlaceAlign};

    fn tokens(line: &str) -> Vec<String> {
        tokenize_line(line).expect("tokenize")
    }

    fn parse(line: &str) -> BatchOp {
        let toks = tokens(line);
        parse_line(1, &toks).expect("parse").expect("active line")
    }

    #[test]
    fn tokenize_line_handles_quotes_comments_and_hash_literals() {
        assert_eq!(tokens(""), Vec::<String>::new());
        assert_eq!(tokens("   "), Vec::<String>::new());
        assert_eq!(tokens("# comment"), Vec::<String>::new());
        assert_eq!(tokens("  # indented comment"), Vec::<String>::new());

        assert_eq!(
            tokens("node --id client --label 'Client App' --x 100"),
            vec![
                "node",
                "--id",
                "client",
                "--label",
                "Client App",
                "--x",
                "100"
            ]
        );

        // \" -> '"', \\ -> '\', \n stays the two characters '\\' and 'n'.
        let label = tokens(r#"--label "Fraud / \"3DS\"\nCheck""#);
        assert_eq!(label, vec!["--label", "Fraud / \"3DS\"\\nCheck"]);
        assert!(
            label[1].contains('\\'),
            "backslash-n must remain an escape for normalize_text, got {:?}",
            label[1]
        );
        assert_eq!(label[1].matches('\\').count(), 1);

        assert_eq!(
            tokens("zone --bg \"#ebfbee\" --title 'Backend'"),
            vec!["zone", "--bg", "#ebfbee", "--title", "Backend"]
        );
        assert_eq!(
            tokens("node --label \"hash # not a comment\" --x 1"),
            vec!["node", "--label", "hash # not a comment", "--x", "1"]
        );

        // A lone backslash before the closer is an incomplete \\ escape pair
        // only when the next char is also a backslash; \" is the quote escape.
        assert_eq!(tokens(r#"--label "a\\b""#), vec!["--label", "a\\b"]);

        let err = tokenize_line(r#"node --label "unterminated"#)
            .expect_err("unclosed double quote")
            .to_string();
        assert_eq!(err, "unclosed quote");
        let err = tokenize_line("node --label 'unterminated")
            .expect_err("unclosed single quote")
            .to_string();
        assert_eq!(err, "unclosed quote");
    }

    #[test]
    fn parse_line_normalizes_prefixes_and_aliases() {
        match parse("node --id n --label L --x 1 --y 2") {
            BatchOp::Draw(cmd) => match *cmd {
                DrawCommands::Node { id, file, .. } => {
                    assert_eq!(id, "n");
                    assert_eq!(file, "<batch>");
                }
                other => panic!("expected Node, got {}", std::any::type_name_of_val(&other)),
            },
            _ => panic!("expected Draw"),
        }

        for line in [
            "draw node --id n --label L --x 1 --y 2",
            "excal draw node --id n --label L --x 1 --y 2",
        ] {
            match parse(line) {
                BatchOp::Draw(cmd) => assert!(
                    matches!(*cmd, DrawCommands::Node { .. }),
                    "{line} did not parse as Node"
                ),
                _ => panic!("{line}: expected Draw"),
            }
        }

        match parse("arrow --from a --to b") {
            BatchOp::Draw(cmd) => assert!(matches!(*cmd, DrawCommands::Connect { .. })),
            _ => panic!("expected Draw Connect"),
        }
        match parse("rm --id a") {
            BatchOp::Draw(cmd) => assert!(matches!(*cmd, DrawCommands::Remove { .. })),
            _ => panic!("expected Draw Remove"),
        }
        match parse("reroute --id a1 --via y=40") {
            BatchOp::Draw(cmd) => assert!(matches!(*cmd, DrawCommands::Route { .. })),
            _ => panic!("expected Draw Route"),
        }

        match parse("align --ids a,b --axis middle") {
            BatchOp::Layout(LayoutCommands::Align {
                ids, axis, file, ..
            }) => {
                assert_eq!(ids, vec!["a", "b"]);
                assert!(matches!(axis, Axis::Middle));
                assert_eq!(file, "<batch>");
            }
            _ => panic!("expected Layout Align"),
        }
        for line in [
            "layout align --ids a,b --axis middle",
            "excal layout align --ids a,b --axis middle",
        ] {
            match parse(line) {
                BatchOp::Layout(LayoutCommands::Align { .. }) => {}
                _ => panic!("{line}: expected Layout Align"),
            }
        }

        let err = parse_line(4, &tokens("init"))
            .expect_err("init rejected")
            .to_string();
        assert!(err.contains("line 4:"), "{err}");
        let err = parse_line(2, &tokens("draw batch --script x"))
            .expect_err("nested batch rejected")
            .to_string();
        assert!(err.contains("line 2:"), "{err}");
        let err = parse_line(7, &tokens("node --id a --x 1 --y 2 --in-place"))
            .expect_err("in-place rejected")
            .to_string();
        assert_eq!(
            err,
            "line 7: output flags (--in-place, -o) are not allowed in batch"
        );
        let err = parse_line(8, &tokens("align --ids a,b --axis left -o out.excalidraw"))
            .expect_err("-o rejected")
            .to_string();
        assert_eq!(
            err,
            "line 8: output flags (--in-place, -o) are not allowed in batch"
        );

        let err = parse_line(3, &tokens("nope --id a"))
            .expect_err("unknown verb")
            .to_string();
        assert!(err.starts_with("line 3:"), "{err}");
        assert!(!err.contains("error: "), "{err}");
    }

    #[test]
    fn execute_script_skips_comments_and_sums_layout_updates() {
        let mut doc = Document::empty();
        let script = "\
# comment

node --id a --label A --x 0 --y 0
node --id b --label B --x 10 --y 40
align --ids a,b --axis middle
";
        let report = execute_script(&mut doc, script, 1_700_000_000_000).expect("execute");
        assert_eq!(report.added, 2);
        assert_eq!(report.updated, 1);
        assert_eq!(report.removed, 0);
        let _ = PlaceAlign::Center;
    }
}
