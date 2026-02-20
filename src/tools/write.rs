use std::error::Error;

pub fn write(file_path: &str, content: &str) -> Result<(), Box<dyn Error>> {
    std::fs::write(file_path, content)?;
    Ok(())
}
