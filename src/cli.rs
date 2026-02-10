use clap::Parser;

const DEFAULT_REVSET: &str = "root() | remote_bookmarks() | ancestors(immutable_heads().., 24)";

#[derive(Parser, Debug)]
#[command(version, about = "Jjdag: A TUI to manipulate the Jujutsu DAG")]
pub struct Args {
    /// Path to repository to operate on
    #[arg(short = 'R', long, default_value = ".")]
    pub repository: String,

    /// Which revisions to show
    #[arg(short = 'r', long, value_name = "REVSETS", default_value = DEFAULT_REVSET)]
    pub revisions: String,
}
