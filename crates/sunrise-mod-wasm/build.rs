#[path = "../../packaging/executable_link.rs"]
mod executable_link;
// The benchmark includes the same fixture root natively, so it needs the identical module list.
#[allow(dead_code)]
#[path = "tests/fixture/build.rs"]
mod fixture_build;

fn main() {
    generate_fixture_readers();
    println!("cargo:rerun-if-changed=../../packaging/executable_link.rs");
    println!("cargo:rerun-if-changed=../sunrise-guest-stack/src/guest_stack_constants.rs");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    for arg in executable_link::arguments(&os, &env) {
        println!("cargo:rustc-link-arg-tests={arg}");
    }
    let root =
        std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    if std::env::var_os("CARGO_FEATURE_LISTENER_FIXTURE").is_some() {
        fixture_build::generate(&root.join("crates/sunrise-mod-wasm/tests/fixture"), &out);
    }
    println!("cargo:rerun-if-changed=../sunrise-mod-api/src");
    println!("cargo:rerun-if-changed=../sunrise-mod-vocab/src");
    println!("cargo:rerun-if-changed=../sunrise-mod-api-gen/src");
    let code = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || sunrise_mod_api_gen::generate_wire_host(&root))
        .expect("starting host generator")
        .join()
        .expect("host generator panicked")
        .expect("generating host boundary");
    sunrise_mod_api_gen::write_if_changed(out.join("wire_host_generated.rs"), code)
        .expect("writing host boundary");
}

// Use staging's emitter so the integration tests exercise the shipped cell reads.
fn generate_fixture_readers() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for (file, readers) in [
        (
            "native_listener_readers.rs",
            &[
                ("__fnr_read_slot_0", 0, "fn(&mut Option<String>, String)"),
                ("__fnr_read_body_0", 1, "fn(&mut Option<String>, String)"),
                ("__fnr_read_slot_2", 2, "fn(u32, u32) -> u32"),
                ("__fnr_read_body_2", 3, "fn(u32, u32) -> u32"),
                ("__fnr_read_slot_4", 4, "fn(f32) -> f32"),
                ("__fnr_read_body_4", 5, "fn(f32) -> f32"),
            ][..],
        ),
        (
            "real_member_readers.rs",
            &[
                ("__fnr_read_slot_0", 0, "fn(f32) -> f32"),
                ("__fnr_read_body_0", 1, "fn(f32) -> f32"),
                ("__fnr_read_slot_2", 2, "fn(u32, u32) -> u32"),
                ("__fnr_read_body_2", 3, "fn(u32, u32) -> u32"),
                ("__fnr_read_slot_4", 4, "fn(&mut Option<String>, String)"),
                ("__fnr_read_body_4", 5, "fn(&mut Option<String>, String)"),
            ][..],
        ),
        (
            "support_checks_readers.rs",
            &[
                ("__fnr_read_slot_6", 6, "fn(u32, &u32) -> u32"),
                ("__fnr_read_body_6", 7, "fn(u32, &u32) -> u32"),
            ][..],
        ),
    ] {
        let mut code = String::new();
        for (name, cell, ty) in readers {
            code.push_str(
                &sunrise_mod_api_gen::replace::reader(
                    &syn::parse_str(name).expect("fixture reader name"),
                    *cell,
                    &ty.parse().expect("fixture function pointer type"),
                )
                .to_string(),
            );
        }
        sunrise_mod_api_gen::write_if_changed(out.join(file), code)
            .expect("writing fixture replacement readers");
    }
}
