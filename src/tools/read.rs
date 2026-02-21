use std::fs;
use std::io::Error;

pub fn read_file(file_path: &str) -> Result<String, Error> {
    if !fs::exists(file_path)? {
        return Ok(format!("The requested file at {} does not exist.", file_path).to_string());
    }

    let contents = fs::read_to_string(file_path)?;
    Ok(contents.to_string())
}
