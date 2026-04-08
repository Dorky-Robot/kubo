//! Verify that expected tools are installed in the kubo image.
//!
//! These tests require a built kubo:latest image. Run `kubo refresh` first.
//! Ignored by default so `cargo test` stays fast — run with:
//!   cargo test --test image_tools -- --ignored

use std::process::Command;

fn tool_exists_in_image(tool: &str) -> bool {
    let output = Command::new("docker")
        .args(["run", "--rm", "kubo:latest", "which", tool])
        .output()
        .expect("failed to run docker");
    output.status.success()
}

macro_rules! tool_test {
    ($name:ident, $tool:expr) => {
        #[test]
        #[ignore]
        fn $name() {
            assert!(
                tool_exists_in_image($tool),
                "'{}' not found in kubo image",
                $tool
            );
        }
    };
}

// CLI tools
tool_test!(has_rg, "rg");
tool_test!(has_fd, "fd");
tool_test!(has_bat, "bat");
tool_test!(has_eza, "eza");
tool_test!(has_fzf, "fzf");
tool_test!(has_delta, "delta");
tool_test!(has_gh, "gh");

// Languages & runtimes
tool_test!(has_rustc, "rustc");
tool_test!(has_cargo, "cargo");
tool_test!(has_node, "node");
tool_test!(has_npm, "npm");
tool_test!(has_go, "go");

// Dorky Robot tools
tool_test!(has_diwa, "diwa");
tool_test!(has_tunnels, "tunnels");
tool_test!(has_cloudflared, "cloudflared");

// Shell
tool_test!(has_zsh, "zsh");
tool_test!(has_tmux, "tmux");
