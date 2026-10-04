use hello_wgpu::renderer::{BLUR_SHADER, GLASS_SHADER};

#[test]
fn wgsl_parses_and_validates() {
    for (name, source) in [("glass", GLASS_SHADER), ("blur", BLUR_SHADER)] {
        let module = naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|error| panic!("{name}: {}", error.emit_to_string(source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}
