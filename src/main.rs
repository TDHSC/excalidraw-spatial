use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use excalidraw_spatial::author::{self, AuthorReport};
use excalidraw_spatial::formatter::ai_report::format_inspect;
use excalidraw_spatial::formatter::ascii_map::render;
use excalidraw_spatial::formatter::{
    format_inspect_json, format_lint_issues_only, format_lint_json, format_lint_text,
};
use excalidraw_spatial::linter::{run_lint, Severity};
use excalidraw_spatial::mutator::{apply, layout_change, strip_nops};
use excalidraw_spatial::scene_graph::Scene;
use excalidraw_spatial::schema::Document;
use excalidraw_spatial::{
    Cli, Commands, DrawCommands, InspectFormat, LayoutCommands, LintFormat, OutputArgs,
};

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::Lint {
            file,
            strict,
            format,
        } => lint_cmd(&file, strict, format),
        Commands::Inspect { file, format } => inspect_cmd(&file, format),
        Commands::Layout { command } => layout_cmd(command),
        Commands::Draw { command } => draw_cmd(*command),
    };
    std::process::exit(code);
}

fn inspect_cmd(file: &str, format: InspectFormat) -> i32 {
    let doc = match Document::load(file) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    let scene = Scene::build(&doc);
    let title = if file == "-" { "<stdin>" } else { file };
    let lint_file = if file == "-" { "<file>" } else { file };
    let out = match format {
        InspectFormat::Ai => {
            let issues = run_lint(&scene, lint_file);
            format_inspect(title, &scene, &issues)
        }
        InspectFormat::Ascii => render(&scene).join("\n") + "\n",
        InspectFormat::Json => {
            let issues = run_lint(&scene, lint_file);
            format_inspect_json(file, &scene, &issues)
        }
    };
    print!("{out}");
    0
}

fn lint_cmd(file: &str, strict: bool, format: LintFormat) -> i32 {
    let doc = match Document::load(file) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    let scene = Scene::build(&doc);
    let lint_file = if file == "-" { "<file>" } else { file };
    let issues = run_lint(&scene, lint_file);
    let out = match format {
        LintFormat::Text => format_lint_text(&issues),
        LintFormat::Json => format_lint_json(file, &issues),
    };
    print!("{out}");
    if issues.iter().any(|i| i.severity == Severity::Error)
        || (strict && issues.iter().any(|i| i.severity == Severity::Warn))
    {
        1
    } else {
        0
    }
}

fn layout_cmd(cmd: LayoutCommands) -> i32 {
    let (file, output) = layout_io(&cmd);
    if file == "-" && output.in_place {
        eprintln!("excal: error: cannot use --in-place with stdin");
        return 2;
    }

    let mut doc = match Document::load(file) {
        Ok(doc) => doc,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    let old_scene = Scene::build(&doc);
    let mut change = match layout_change(&old_scene, &cmd) {
        Ok(change) => change,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    strip_nops(&mut change, &old_scene);

    if change.is_empty() {
        eprintln!("excal: nothing to change");
        return write_output(file, output, &doc);
    }

    let report = match apply(&mut doc, &old_scene, &change) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    let now_ms = match now_ms() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    doc.finish(now_ms);

    let new_scene = Scene::build(&doc);
    let lint_file = if file == "-" { "<file>" } else { file };
    let issues = run_lint(&new_scene, lint_file);
    let mut errors = 0;
    let mut warnings = 0;
    let mut info = 0;
    for issue in &issues {
        match issue.severity {
            Severity::Error => errors += 1,
            Severity::Warn => warnings += 1,
            Severity::Info => info += 1,
        }
    }
    eprintln!(
        "excal: moved {} item(s), resized {} item(s), updated {} connector(s)",
        report.moved, report.resized, report.connectors
    );
    eprintln!("excal: lint after change: errors: {errors}, warnings: {warnings}, info: {info}");
    write_output(file, output, &doc)
}

fn draw_cmd(cmd: DrawCommands) -> i32 {
    let (file, output, init_force) = match &cmd {
        DrawCommands::Init {
            file,
            force,
            output,
        } => (file.as_str(), output, *force),
        DrawCommands::Node { file, output, .. }
        | DrawCommands::Connect { file, output, .. }
        | DrawCommands::Route { file, output, .. }
        | DrawCommands::Zone { file, output, .. }
        | DrawCommands::Frame { file, output, .. }
        | DrawCommands::Text { file, output, .. }
        | DrawCommands::Label { file, output, .. }
        | DrawCommands::Remove { file, output, .. }
        | DrawCommands::Batch { file, output, .. } => (file.as_str(), output, false),
    };
    if file == "-" && output.in_place {
        eprintln!("excal: error: cannot use --in-place with stdin");
        return 2;
    }

    if matches!(cmd, DrawCommands::Init { .. }) {
        return draw_init(file, output, init_force);
    }

    eprintln!("excal: error: not implemented");
    2
}

fn draw_init(file: &str, output: &OutputArgs, force: bool) -> i32 {
    let doc = Document::empty();
    let target = output
        .output
        .as_deref()
        .or(if file == "-" { None } else { Some(file) });
    if let Some(path) = target {
        if !force && std::path::Path::new(path).exists() {
            eprintln!("excal: error: file \"{path}\" already exists; pass --force to overwrite");
            return 2;
        }
    }

    let now_ms = match now_ms() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    let mut doc = doc;
    let report = match author::apply_draw(
        &mut doc,
        &DrawCommands::Init {
            file: file.to_string(),
            force,
            output: OutputArgs {
                in_place: output.in_place,
                output: output.output.clone(),
            },
        },
        now_ms,
    ) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("excal: error: {e}");
            return 2;
        }
    };
    doc.finish(now_ms);

    let scene = Scene::build(&doc);
    let lint_file = target.unwrap_or("<file>");
    emit_draw_stderr(&report, &scene, lint_file);
    write_draw_output(
        file,
        output,
        target.is_some() && output.output.is_none(),
        &doc,
    )
}

fn emit_draw_stderr(report: &AuthorReport, scene: &Scene, lint_file: &str) {
    let issues = run_lint(scene, lint_file);
    let mut errors = 0;
    let mut warnings = 0;
    let mut info = 0;
    for issue in &issues {
        match issue.severity {
            Severity::Error => errors += 1,
            Severity::Warn => warnings += 1,
            Severity::Info => info += 1,
        }
    }
    eprintln!(
        "excal: added {} item(s), updated {} item(s), removed {} item(s)",
        report.added, report.updated, report.removed
    );
    eprintln!("excal: lint after change: errors: {errors}, warnings: {warnings}, info: {info}");
    if !issues.is_empty() {
        eprint!("{}", format_lint_issues_only(&issues));
    }
}

fn write_draw_output(file: &str, output: &OutputArgs, write_file: bool, doc: &Document) -> i32 {
    let json = doc.to_json_string();
    let result = if output.in_place {
        std::fs::write(file, json)
    } else if let Some(path) = &output.output {
        std::fs::write(path, json)
    } else if write_file {
        std::fs::write(file, json)
    } else {
        print!("{json}");
        return 0;
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("excal: error: {e}");
            2
        }
    }
}

fn layout_io(cmd: &LayoutCommands) -> (&str, &OutputArgs) {
    match cmd {
        LayoutCommands::Move { file, output, .. }
        | LayoutCommands::Align { file, output, .. }
        | LayoutCommands::Distribute { file, output, .. }
        | LayoutCommands::Place { file, output, .. }
        | LayoutCommands::FitContainer { file, output, .. }
        | LayoutCommands::FitText { file, output, .. } => (file, output),
    }
}

fn now_ms() -> anyhow::Result<i64> {
    if let Ok(v) = std::env::var("EXCAL_FIXED_NOW_MS") {
        Ok(v.parse::<i64>()?)
    } else {
        Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as i64)
    }
}

fn write_output(file: &str, output: &OutputArgs, doc: &Document) -> i32 {
    let json = doc.to_json_string();
    let result = if output.in_place {
        std::fs::write(file, json)
    } else if let Some(path) = &output.output {
        std::fs::write(path, json)
    } else {
        print!("{json}");
        return 0;
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("excal: error: {e}");
            2
        }
    }
}
