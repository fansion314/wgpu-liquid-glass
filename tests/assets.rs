use hello_wgpu::scene;

#[test]
fn embedded_pictures_load_at_original_resolution_before_first_frame() {
    let ctx = scene::context();
    let pictures = scene::images(&ctx, 8192).unwrap();
    assert_eq!(pictures[0].dimensions, [2048, 858]);
    assert_eq!(pictures[1].dimensions, [6000, 4000]);
    assert_eq!(pictures[2].dimensions, [1080, 1920]);
}

#[test]
fn local_png_is_loaded_and_fitted_to_gpu_limit() {
    let path = std::env::temp_dir().join(format!("hello-wgpu-image-{}.png", std::process::id()));
    image::RgbaImage::from_pixel(120, 80, image::Rgba([20, 70, 120, 255]))
        .save(&path)
        .unwrap();
    let ctx = scene::context();
    let picture = scene::local_image(&ctx, 64, &path).unwrap();
    assert_eq!(picture.dimensions, [64, 43]);
    assert!(picture.credit.contains("已适配 GPU"));
    assert!(picture.source.is_none());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn invalid_local_image_returns_error_without_allocating_texture() {
    let path = std::env::temp_dir().join(format!("hello-wgpu-invalid-{}.png", std::process::id()));
    std::fs::write(&path, b"not an image").unwrap();
    let ctx = scene::context();
    assert!(scene::local_image(&ctx, 8192, &path).is_err());
    std::fs::remove_file(path).unwrap();
}
