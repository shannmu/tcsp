use std::process::Command;

/// Check if the current process has root privilege
pub(crate) fn has_root_privilege() -> bool{
    match Command::new("id").arg("-u").output() {
        Ok(output) => {
            let uid = String::from_utf8_lossy(&output.stdout);
            uid.trim() == "0"
        }
        Err(_) => {
            false
        }
    }
}
