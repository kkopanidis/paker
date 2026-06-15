use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrefixStat {
    pub prefix: String,
    pub object_count: u64,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BucketReport {
    pub total_objects: u64,
    pub total_bytes: u64,
    pub top_prefixes_by_bytes: Vec<PrefixStat>,
    pub glacier_object_count: u64,
    pub glacier_bytes: u64,
    pub small_file_count: u64,
    pub small_file_threshold_bytes: u64,
}
