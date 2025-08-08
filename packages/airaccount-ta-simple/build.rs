use optee_utee_build::{TaConfig, RustEdition, Error};

const UUID: &str = "11223344-5566-7788-99aa-bbccddeeff01";

fn main() -> Result<(), Error> {
    let config = TaConfig::new_default(
        UUID,
        "0.1.0",
        "AirAccount Simple TA for testing"
    )?;
    optee_utee_build::build(RustEdition::Before2024, config)
}