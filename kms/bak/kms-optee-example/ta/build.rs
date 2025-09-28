use proto::{Command};
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

const TA_UUID: &str = "8aaaf200-2450-11e4-abe2-0002a5d5c51b";
const TA_PROPERTIES: &str = "
gpd.ta.appID = {ta_uuid}
gpd.ta.singleInstance = true
gpd.ta.multiSession = true
gpd.ta.instanceKeepAlive = false
gpd.ta.dataSize = 65536
gpd.ta.stackSize = 65536
gpd.ta.version = 1.0
gpd.ta.description = KMS Trusted Application
";

fn main() -> std::io::Result<()> {
    let out_dir = env::var("OUT_DIR").unwrap();

    let dest_path = PathBuf::from(&out_dir).join("user_ta_header.rs");
    let mut f = File::create(&dest_path)?;

    let ta_properties = TA_PROPERTIES.replace("{ta_uuid}", TA_UUID);

    f.write_all(format!(
        r#"
const TA_UUID: optee_utee_sys::TEE_UUID = optee_utee_sys::TEE_UUID {{
    timeLow: 0x8aaaf200,
    timeMid: 0x2450,
    timeHiAndVersion: 0x11e4,
    clockSeqAndNode: [0xab, 0xe2, 0x00, 0x02, 0xa5, 0xd5, 0xc5, 0x1b],
}};

const TA_FLAGS: u32 = 0;
const TA_STACK_SIZE: u32 = 65536;
const TA_DATA_SIZE: u32 = 65536;
const TA_VERSION: &[u8] = b"1.0\0";
const TA_DESCRIPTION: &[u8] = b"KMS Trusted Application\0";
const EXT_PROP_VALUE_1: &[u8] = b"KMS TA\0";
const EXT_PROP_VALUE_2: &[u8] = b"1.0\0";
const TRACE_LEVEL: i32 = 4;
const TRACE_EXT_PREFIX: &[u8] = b"TA\0";
const TA_FRAMEWORK_STACK_SIZE: u32 = 2048;

#[no_mangle]
pub static user_ta_header: optee_utee_sys::user_ta_header_t = optee_utee_sys::user_ta_header_t {{
    uuid: TA_UUID,
    prop: 0 as *mut optee_utee_sys::user_ta_property_t,
    num_props: 0,
    entry_func: 0 as *mut core::ffi::c_void,
    stack_size: TA_STACK_SIZE,
    flags: TA_FLAGS,
    depr_entry_func: 0 as *mut core::ffi::c_void,
}};
"#
    ).as_bytes())?;

    optee_utee_build::compile_ta_properties(TA_UUID, &ta_properties)?;

    println!("cargo:rerun-if-changed=build.rs");
    Ok(())
}