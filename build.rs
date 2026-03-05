use std::path::Path;

fn main() {
    // Ensure web/dist/ exists so rust-embed compiles even without a prior web build.
    let dist = Path::new("web/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).expect("failed to create web/dist directory");
    }
}
