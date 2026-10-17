use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

pub mod author;
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
    Draw {
        #[command(subcommand)]
        command: Box<DrawCommands>,
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
        #[arg(long)]
        expand_only: bool,
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

#[derive(Clone, ValueEnum)]
pub enum DrawShape {
    Rectangle,
    Ellipse,
    Diamond,
    Stickynote,
}

#[derive(Clone, ValueEnum)]
pub enum StrokeStyle {
    Solid,
    Dashed,
    Dotted,
}

#[derive(Clone, ValueEnum)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Subcommand)]
pub enum DrawCommands {
    Init {
        file: String,
        #[arg(long)]
        force: bool,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(group(
        ArgGroup::new("placement")
            .required(false)
            .multiple(false)
            .args(["x", "right_of", "left_of", "below", "above"])
    ))]
    Node {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long, visible_alias = "text")]
        label: Option<String>,
        #[arg(long, value_enum, default_value_t = DrawShape::Rectangle)]
        shape: DrawShape,
        #[arg(long, allow_negative_numbers = true, requires = "y")]
        x: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        y: Option<f64>,
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
        #[arg(long, value_enum)]
        align: Option<PlaceAlign>,
        #[arg(id = "in_container", long = "in")]
        in_container: Option<String>,
        #[arg(long)]
        from: Option<String>,
        #[arg(long, requires = "from")]
        edge_label: Option<String>,
        #[arg(long)]
        elbow: bool,
        #[arg(long, allow_negative_numbers = true)]
        width: Option<f64>,
        #[arg(long, allow_negative_numbers = true)]
        height: Option<f64>,
        #[arg(long)]
        bg: Option<String>,
        #[arg(long)]
        stroke: Option<String>,
        #[arg(long, value_enum)]
        stroke_style: Option<StrokeStyle>,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(visible_alias = "arrow")]
    Connect {
        file: String,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long, visible_alias = "text")]
        label: Option<String>,
        #[arg(long)]
        elbow: bool,
        #[arg(long)]
        via: Option<String>,
        #[arg(long, value_enum)]
        from_side: Option<Side>,
        #[arg(long, value_enum)]
        to_side: Option<Side>,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(visible_alias = "reroute")]
    Route {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        via: String,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(group(
        ArgGroup::new("zone_mode")
            .required(true)
            .multiple(false)
            .args(["children", "x"])
    ))]
    Zone {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long, visible_aliases = ["label", "text"])]
        title: String,
        #[arg(long, value_delimiter = ',')]
        children: Vec<String>,
        #[arg(long, allow_negative_numbers = true, requires = "children")]
        padding: Option<f64>,
        #[arg(
            long,
            allow_negative_numbers = true,
            requires = "y",
            requires = "width",
            requires = "height"
        )]
        x: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        y: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        width: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        height: Option<f64>,
        #[arg(id = "in_container", long = "in")]
        in_container: Option<String>,
        #[arg(long)]
        bg: Option<String>,
        #[arg(long)]
        stroke: Option<String>,
        #[arg(long, value_enum)]
        stroke_style: Option<StrokeStyle>,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(group(
        ArgGroup::new("frame_mode")
            .required(true)
            .multiple(false)
            .args(["children", "x"])
    ))]
    Frame {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long, visible_aliases = ["label", "title"])]
        name: Option<String>,
        #[arg(long, value_delimiter = ',')]
        children: Vec<String>,
        #[arg(long, allow_negative_numbers = true, requires = "children")]
        padding: Option<f64>,
        #[arg(
            long,
            allow_negative_numbers = true,
            requires = "y",
            requires = "width",
            requires = "height"
        )]
        x: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        y: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        width: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        height: Option<f64>,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(group(
        ArgGroup::new("placement")
            .required(false)
            .multiple(false)
            .args(["x", "right_of", "left_of", "below", "above"])
    ))]
    Text {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long, visible_alias = "label")]
        text: String,
        #[arg(long, allow_negative_numbers = true, requires = "y")]
        x: Option<f64>,
        #[arg(long, allow_negative_numbers = true, requires = "x")]
        y: Option<f64>,
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
        #[arg(long, value_enum)]
        align: Option<PlaceAlign>,
        #[arg(long, allow_negative_numbers = true)]
        font_size: Option<f64>,
        #[command(flatten)]
        output: OutputArgs,
    },
    Label {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long, visible_alias = "label")]
        text: String,
        #[command(flatten)]
        output: OutputArgs,
    },
    #[command(visible_aliases = ["rm", "delete"])]
    Remove {
        file: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        keep_arrows: bool,
        #[command(flatten)]
        output: OutputArgs,
    },
    Batch {
        file: String,
        #[arg(long)]
        script: Option<String>,
        #[command(flatten)]
        output: OutputArgs,
    },
}
