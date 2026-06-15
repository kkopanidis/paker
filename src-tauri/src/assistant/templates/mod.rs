mod aws_cli;
mod rclone;

pub use aws_cli::generate_aws_cli;
pub use rclone::generate_rclone;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliGenerateInput {
    pub tool: Option<String>,
    pub connection_id: String,
    pub connection_name: Option<String>,
    pub endpoint: Option<String>,
    pub bucket: String,
    pub prefix: Option<String>,
    pub keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliCommandSuggestion {
    pub tool: String,
    pub command: String,
    pub description: String,
}

pub fn generate_cli_commands(input: &CliGenerateInput) -> Vec<CliCommandSuggestion> {
    let mut out = Vec::new();
    let want_aws = input
        .tool
        .as_deref()
        .is_none_or(|t| t.eq_ignore_ascii_case("aws"));
    let want_rclone = input
        .tool
        .as_deref()
        .is_none_or(|t| t.eq_ignore_ascii_case("rclone"));

    if want_aws {
        out.extend(generate_aws_cli(input));
    }
    if want_rclone {
        out.extend(generate_rclone(input));
    }
    out
}
