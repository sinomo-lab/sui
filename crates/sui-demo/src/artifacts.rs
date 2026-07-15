#[cfg(not(target_arch = "wasm32"))]
fn main() -> sui::Result<()> {
    let output_dir = sui_demo_app::widget_book::write_visual_artifacts()?;
    println!("Wrote widget-book artifacts to {}", output_dir.display());
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
