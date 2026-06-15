use super::CliCommandSuggestion;
use super::CliGenerateInput;

pub fn generate_rclone(input: &CliGenerateInput) -> Vec<CliCommandSuggestion> {
    let remote = sanitize_remote_name(
        input
            .connection_name
            .as_deref()
            .unwrap_or("paker-remote"),
    );
    let mut out = Vec::new();

    if input.keys.is_empty() {
        let prefix = input.prefix.as_deref().unwrap_or("");
        let path = if prefix.is_empty() {
            format!("{remote}:{}/", input.bucket)
        } else {
            format!("{remote}:{}/{prefix}", input.bucket)
        };
        out.push(CliCommandSuggestion {
            tool: "rclone".to_string(),
            command: format!("rclone ls {path}"),
            description: "List objects (requires rclone remote configured)".to_string(),
        });
        out.push(CliCommandSuggestion {
            tool: "rclone".to_string(),
            command: format!("rclone sync {path} ./local-copy"),
            description: "Sync prefix to local directory".to_string(),
        });
        return out;
    }

    for key in &input.keys {
        out.push(CliCommandSuggestion {
            tool: "rclone".to_string(),
            command: format!("rclone copy {remote}:{}/{} .", input.bucket, key),
            description: format!("Copy `{key}` locally"),
        });
    }

    out
}

fn sanitize_remote_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c
            } else {
                '-'
            }
        })
        .collect()
}
