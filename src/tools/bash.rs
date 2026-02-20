use std::error::Error;
use std::process::Command;

pub fn execute_bash_command(command: &str) -> Result<String, Box<dyn Error>> {
    // Prevent running any command containing 'rm'
    if command.contains("rm") {
        return Err("Command contains 'rm' and is not allowed to run.".into());
    }

    let output = Command::new("bash").args(["-c", command]).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        return Ok(stdout);
    }

    let exit_code = output.status.code().unwrap_or(-1);
    let mut error_output = format!("Command failed with exit code {exit_code}");
    if !stderr.is_empty() {
        error_output.push('\n');
        error_output.push_str(&stderr);
    }
    if !stdout.is_empty() {
        error_output.push('\n');
        error_output.push_str(&stdout);
    }
    Err(error_output.into())
}