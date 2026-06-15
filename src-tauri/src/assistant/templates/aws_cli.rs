use super::super::templates::CliGenerateInput;
use super::super::templates::CliCommandSuggestion;

pub fn generate_aws_cli(input: &CliGenerateInput) -> Vec<CliCommandSuggestion> {
    let mut out = Vec::new();
    let endpoint_flag = input
        .endpoint
        .as_ref()
        .map(|e| format!(" --endpoint-url {e}"))
        .unwrap_or_default();

    if input.keys.is_empty() {
        let prefix = input.prefix.as_deref().unwrap_or("");
        let s3_uri = if prefix.is_empty() {
            format!("s3://{}/", input.bucket)
        } else {
            format!("s3://{}/{prefix}", input.bucket)
        };
        out.push(CliCommandSuggestion {
            tool: "aws".to_string(),
            command: format!("aws s3 ls {s3_uri}{endpoint_flag}"),
            description: "List objects under the current prefix".to_string(),
        });
        out.push(CliCommandSuggestion {
            tool: "aws".to_string(),
            command: format!("aws s3 sync {s3_uri} ./local-copy{endpoint_flag}"),
            description: "Sync prefix to a local folder".to_string(),
        });
        return out;
    }

    if input.keys.len() == 1 {
        let key = &input.keys[0];
        out.push(CliCommandSuggestion {
            tool: "aws".to_string(),
            command: format!(
                "aws s3 cp s3://{}/{key} .{endpoint_flag}",
                input.bucket
            ),
            description: "Download a single object".to_string(),
        });
    } else {
        for key in &input.keys {
            out.push(CliCommandSuggestion {
                tool: "aws".to_string(),
                command: format!(
                    "aws s3 cp s3://{}/{key} .{endpoint_flag}",
                    input.bucket
                ),
                description: format!("Download `{key}`"),
            });
        }
    }

    out
}
