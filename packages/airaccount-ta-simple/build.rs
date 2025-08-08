use optee_utee_build::*;
use uuid::Uuid;

const UUID: &str = "11223344-5566-7788-99aa-bbccddeeff01"; // 稍微不同的UUID避免冲突

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let uuid = Uuid::parse_str(UUID)?;

    optee_utee_build::ta_builder()
        .uuid(uuid.as_bytes())
        .single_instance()
        .build()?;

    Ok(())
}