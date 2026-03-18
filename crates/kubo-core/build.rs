use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

fn main() {
    let image_dir = Path::new("../../image");
    let files = &[
        "Dockerfile",
        "entrypoint.sh",
        "zshrc",
        "kubo.zsh-theme",
        "tmux.conf",
        "vimrc",
        "welcome.sh",
        "pbpaste",
        "pbcopy",
        "clip",
    ];

    let mut hasher = DefaultHasher::new();
    for name in files {
        let path = image_dir.join(name);
        if let Ok(content) = fs::read(&path) {
            content.hash(&mut hasher);
        }
        // Rerun if any image file changes
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let hash = hasher.finish();
    // Use cargo version + short hash for the image version
    let pkg_version = env!("CARGO_PKG_VERSION");
    println!("cargo:rustc-env=KUBO_IMAGE_HASH={pkg_version}-{hash:016x}");
}
