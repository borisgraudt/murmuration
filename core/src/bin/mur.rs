fn main() -> anyhow::Result<()> {
    murmuration::cli_app::run(std::env::args().collect())
}
