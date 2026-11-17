//! demo02's tutorial runner. The runner itself lives in `../runner` and is
//! shared by every demo; this file only hands it demo02's exercise list.
//! The conversation the wire probe holds with a server is in `info.toml`.

fn main() -> std::process::ExitCode {
    demo_runner::main(demo_runner::Demo {
        info_toml: include_str!("../info.toml"),
        manifest_dir: env!("CARGO_MANIFEST_DIR"),
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
    })
}
