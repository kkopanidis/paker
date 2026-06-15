use crate::assistant::builders::ActionKind;

pub const MAX_OBJECTS_HARD_LIMIT: usize = 10_000;
/// Index age beyond which a destructive action is considered stale (24 h).
pub const STALE_INDEX_AGE_SECS: u64 = 86_400;

#[derive(Debug, Default)]
pub struct PolicyResult {
    /// Hard-stop violation messages (non-empty → builder must abort).
    pub violations: Vec<String>,
    /// Soft warning messages (surfaced in the review dialog).
    pub warnings: Vec<String>,
}

impl PolicyResult {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn warning_messages(&self) -> Vec<String> {
        self.warnings.clone()
    }
}

pub struct PolicyContext {
    pub kind: ActionKind,
    pub affected_count: usize,
    pub has_glacier: bool,
    pub bucket_versioned: bool,
    pub index_age_secs: Option<u64>,
}

pub fn check(ctx: &PolicyContext) -> PolicyResult {
    let mut result = PolicyResult::default();

    // Hard limit: too many objects
    if ctx.affected_count > MAX_OBJECTS_HARD_LIMIT {
        result.violations.push(format!(
            "proposal would affect {} objects; limit is {}",
            ctx.affected_count, MAX_OBJECTS_HARD_LIMIT
        ));
    }

    // Stale index check for destructive/mutative operations
    let is_mutative = matches!(ctx.kind, ActionKind::DeleteByQuery | ActionKind::RenamePattern);
    if is_mutative {
        match ctx.index_age_secs {
            None => {
                result.violations.push(
                    "bucket index is stale or absent — re-index before running destructive actions"
                        .to_string(),
                );
            }
            Some(age) if age > STALE_INDEX_AGE_SECS => {
                result.violations.push(format!(
                    "bucket index is {age}s old (limit {STALE_INDEX_AGE_SECS}s); re-index before running destructive actions"
                ));
            }
            _ => {}
        }
    }

    // Dry-run environment check
    if is_mutative {
        if std::env::var("PAKER_POLICY_DRY_RUN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            result.violations.push(
                "dry-run mode active (PAKER_POLICY_DRY_RUN=1); no mutations allowed".to_string(),
            );
        }
    }

    // Glacier warning
    if ctx.has_glacier {
        result.warnings.push(
            "Some objects are in GLACIER or DEEP_ARCHIVE storage class. \
             Restoring them may incur additional costs."
                .to_string(),
        );
    }

    // Versioned bucket warning for deletes
    if ctx.bucket_versioned && matches!(ctx.kind, ActionKind::DeleteByQuery) {
        result.warnings.push(
            "This bucket has versioning enabled. \
             Only current object versions will be deleted; previous versions remain."
                .to_string(),
        );
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_objects_hard_limit_fires_at_10001() {
        let ctx = PolicyContext {
            kind: ActionKind::DeleteByQuery,
            affected_count: 10_001,
            has_glacier: false,
            bucket_versioned: false,
            index_age_secs: Some(60),
        };
        let result = check(&ctx);
        assert!(!result.is_clean());
        assert!(result.violations[0].contains("10001"));
    }

    #[test]
    fn exactly_at_limit_is_allowed() {
        let ctx = PolicyContext {
            kind: ActionKind::DeleteByQuery,
            affected_count: 10_000,
            has_glacier: false,
            bucket_versioned: false,
            index_age_secs: Some(60),
        };
        let result = check(&ctx);
        // Only stale-index might fire, not too-many-objects
        assert!(!result.violations.iter().any(|v| v.contains("10000") || v.contains("limit")));
    }

    #[test]
    fn glacier_warning_attached_correctly() {
        let ctx = PolicyContext {
            kind: ActionKind::DeleteByQuery,
            affected_count: 5,
            has_glacier: true,
            bucket_versioned: false,
            index_age_secs: Some(60),
        };
        let result = check(&ctx);
        assert!(result.warnings.iter().any(|w| w.contains("GLACIER")));
    }

    #[test]
    fn stale_index_blocks_destructive_actions() {
        let ctx = PolicyContext {
            kind: ActionKind::DeleteByQuery,
            affected_count: 5,
            has_glacier: false,
            bucket_versioned: false,
            index_age_secs: None,
        };
        let result = check(&ctx);
        assert!(!result.is_clean());
        assert!(result.violations.iter().any(|v| v.contains("stale")));
    }

    #[test]
    fn stale_index_does_not_block_sync_plan() {
        let ctx = PolicyContext {
            kind: ActionKind::SyncPlan,
            affected_count: 5,
            has_glacier: false,
            bucket_versioned: false,
            index_age_secs: None,
        };
        let result = check(&ctx);
        assert!(result.is_clean());
    }

    #[test]
    fn versioned_bucket_warning_on_delete() {
        let ctx = PolicyContext {
            kind: ActionKind::DeleteByQuery,
            affected_count: 3,
            has_glacier: false,
            bucket_versioned: true,
            index_age_secs: Some(60),
        };
        let result = check(&ctx);
        assert!(result.warnings.iter().any(|w| w.contains("versioning")));
    }
}
