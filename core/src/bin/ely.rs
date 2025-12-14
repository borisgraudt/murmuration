fn main() -> anyhow::Result<()> {
    meshlink_core::cli_app::run(std::env::args().collect())
}


