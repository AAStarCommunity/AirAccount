use optee_utee_build::{Error, RustEdition, TaConfig};

fn main() -> Result<(), Error> {
    let ta_config = TaConfig::new_default_with_cargo_env("11223344-5566-7788-99aa-bbccddeeff00")?
        .ta_data_size(1024 * 1024)
        .ta_stack_size(128 * 1024);
    optee_utee_build::build(RustEdition::Before2024, ta_config)
}