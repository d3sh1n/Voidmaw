use windows::Win32::Foundation::GetLastError;

/// Log a Windows API function failure with error code
pub fn log_winapi_fail(func_name: &str) {
    unsafe {
        let error_code = GetLastError();
        println!("[X] ERROR - WINAPI {} failed with error code: 0x{:x}", func_name, error_code.0);
    }
}

/// Log a general program failure message
pub fn log_program_fail(message: &str) {
    println!("[X] FAIL - {}", message);
}

/// Log a success message
pub fn log_program_success(message: &str) {
    println!("[+] SUCCESS - {}", message);
}

/// Log an informational message
pub fn log_program_message(message: &str) {
    println!("[~] LOG - {}", message);
}

/// Convert a u32 to a hexadecimal string
pub fn dword_to_hex_string(value: u32) -> String {
    format!("{:x}", value)
}

/// Convert a u32 to a decimal string
pub fn int_to_string(value: u32) -> String {
    format!("{}", value)
}
