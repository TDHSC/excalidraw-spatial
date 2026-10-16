use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

pub mod formatter;
pub mod geometry;
pub mod linter;
pub mod mutator;
pub mod scene_graph;
pub mod schema;
pub mod spatial;
pub mod text_metrics;

pub fn not_implemented() -> i32 {
    eprintln!("excal: error: not implemented");
    2
}

#[derive(Parser)]
#[command(name = "excal")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    Inspect {
        file: String,
        #[arg(long, value_enum, default_value_t = InspectFormat::Ai)]
        format: InspectFormat,
    },
    Lint {
        file: String,
        #[arg(long)]
        strict: bool,
        #[arg(long, value_enum, default_value_t = LintFormat::Text)]
        format: LintFormat,
    },
    Layout {
        #[command(subcommand)]
        command: LayoutCommands,
    },
}

#[derive(Clone, ValueEnum)]
pub enum InspectFormat {
    Ai,
    Json,
    Ascii,
}

#[derive(Clone, ValueEnum)]
pub enum LintFormat {
    Text,
    Json,
}

#[derive(Args)]
#[group(id = "output_dest", required = false, multiple = false)]
pub struct OutputArgs {
    #[arg(long)]
    pub in_place: bool,
    #[arg(short = 'o')]
    pub output: Option<String>,
}

#[derive(Clone, ValueEnum)]
pub enum Axis {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}

#[derive(Clone, ValueEnum)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Clone, ValueEnum)]
pub enum PlaceAlign {
    Start,
    Center,
    End,
}

#[derive(Subcommand)]
pub enum LayoutCommands {
    Move {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long, allow_negative_numbers = true)]
        dx: Option<f64>,
        #[arg(long, allow_negative_numbers = true)]
        dy: Option<f64>,
        #[command(flatten)]
        output: OutputArgs,
    },
    Align {
        file: String,
        #[arg(long, required = true, value_delimiter = ',')]
        ids: Vec<String>,
        #[arg(long, value_enum)]
        axis: Axis,
        #[command(flatten)]
        output: OutputArgs,
    },
    Distribute {
        file: String,
        #[arg(long, required = true, value_delimiter = ',')]
        ids: Vec<String>,
        #[arg(long, value_enum)]
        direction: Direction,
        #[arg(long, allow_negative_numbers = true)]
        gap: Option<f64>,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(group(
        ArgGroup::new("place_direction")
            .required(true)
            .args(["right_of", "left_of", "below", "above"])
    ))]
    Place {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        right_of: Option<String>,
        #[arg(long)]
        left_of: Option<String>,
        #[arg(long)]
        below: Option<String>,
        #[arg(long)]
        above: Option<String>,
        #[arg(long, allow_negative_numbers = true)]
        gap: Option<f64>,
        #[arg(long, value_enum, default_value_t = PlaceAlign::Center)]
        align: PlaceAlign,
        #[command(flatten)]
        output: OutputArgs,
    },
    FitContainer {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long, allow_negative_numbers = true)]
        padding: Option<f64>,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(group(
        ArgGroup::new("fit_text_target")
            .required(true)
            .args(["id", "all"])
    ))]
    FitText {
        file: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        all: bool,
        #[arg(long, allow_negative_numbers = true)]
        padding_x: Option<f64>,
        #[arg(long, allow_negative_numbers = true)]
        padding_y: Option<f64>,
        #[command(flatten)]
        output: OutputArgs,
    },
}
