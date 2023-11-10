use std::fs;
use std::io::Write;

const TARGET_ARCH_ENV: &str = "CARGO_CFG_TARGET_ARCH";

fn main() {
    println!("cargo:rerun-if-env-changed={}", TARGET_ARCH_ENV);
    let base_addr: usize = {
        match std::env::var_os(TARGET_ARCH_ENV).unwrap().to_str().unwrap() {
            "riscv32" => 0x8040_0000,
            "riscv64" => 0xFFFF_FFC0_8020_0000,
            other => panic!("Unsupported arch: {}", other),
        }
    };
    let mut ld_inc = fs::File::create("tgt/vars.ld-inc").unwrap();
    writeln!(
        ld_inc,
        "/* DO NOT EDIT: This file is generated by build.rs */"
    )
    .unwrap();
    writeln!(ld_inc, "PROVIDE_HIDDEN(BASE_ADDRESS = {:#x});", base_addr).unwrap();
}
