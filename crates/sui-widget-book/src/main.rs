fn main() -> sui::Result<()> {
    let output_dir = sui_widget_book::write_visual_artifacts()?;
    println!("Wrote widget-book artifacts to {}", output_dir.display());
    Ok(())
}