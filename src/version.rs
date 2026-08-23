pub const GIT_COMMIT: &str = env!("LOCUS_GIT_COMMIT");

pub fn display() -> String {
    format!(
        "locus-desk {} (commit {}, schema {})",
        env!("CARGO_PKG_VERSION"),
        GIT_COMMIT,
        crate::db::latest_schema_version()
    )
}
