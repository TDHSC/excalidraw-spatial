use clap::Parser;
use excalidraw_spatial::formatter::{format_lint_json, format_lint_text};
use excalidraw_spatial::linter::{run_lint, Severity};
use excalidraw_spatial::scene_graph::Scene;
use excalidraw_spatial::schema::Document;
use excalidraw_spatial::{not_implemented, Cli, Commands, LintFormat};

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::Lint {
            file,
            strict,
            format,
        } => lint_cmd(&file, strict, format),
        Commands::Inspect { .. } | Commands::Layout { .. } => not_implemented(),
    };
    std::process::exit(code);
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
