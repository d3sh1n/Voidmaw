use windows::Win32::Foundation::{HANDLE, CloseHandle, GetLastError, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileA, ReadFile, WriteFile, GetFileSizeEx,
    FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
    OPEN_EXISTING, CREATE_NEW,
};
use windows::Win32::System::Memory::{VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32First, Process32Next,
    PROCESSENTRY32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    OpenProcess, TerminateProcess, GetCurrentProcessId,
    PROCESS_TERMINATE,
};
use windows::Win32::Security::Cryptography::{
    CryptAcquireContextA, CryptGenRandom, CryptReleaseContext,
    PROV_RSA_AES, CRYPT_VERIFYCONTEXT,
};
use windows::core::{PCSTR, s};
use std::ptr::null_mut;
use indexmap::IndexMap;
use crate::utils::*;
use crate::veh::ASM_INT3;

/// Check if the input file path is valid
pub unsafe fn check_program_args(input_file_path: &str) -> Option<HANDLE> {
    let file_path_cstr = format!("{}\0", input_file_path);
    let handle = CreateFileA(
        PCSTR(file_path_cstr.as_ptr()),
        FILE_GENERIC_READ.0,
        Default::default(),
        None,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        None,
    );

    match handle {
        Ok(h) if h != INVALID_HANDLE_VALUE => Some(h),
        _ => {
            log_winapi_fail("CreateFileA");
            let error = GetLastError();
            if error.0 == 2 {  // ERROR_FILE_NOT_FOUND
                log_program_fail("Invalid argument (ERROR_FILE_NOT_FOUND)");
            }
            None
        }
    }
}

/// Read the input file into memory
pub unsafe fn read_input_file(handle: HANDLE) -> Option<(*mut u8, u32)> {
    let mut file_size = 0i64;
    if GetFileSizeEx(handle, &mut file_size).is_err() {
        let _ = CloseHandle(handle);
        log_winapi_fail("GetFileSizeEx");
        return None;
    }

    let payload_size = file_size as u32;
    let asm_from_file = VirtualAlloc(
        None,
        payload_size as usize,
        MEM_COMMIT,
        PAGE_READWRITE,
    );

    if asm_from_file.is_null() {
        let _ = CloseHandle(handle);
        log_winapi_fail("VirtualAlloc");
        return None;
    }

    let mut bytes_read = 0;
    if ReadFile(
        handle,
        Some(std::slice::from_raw_parts_mut(asm_from_file as *mut u8, payload_size as usize)),
        Some(&mut bytes_read),
        None,
    ).is_err() {
        let _ = CloseHandle(handle);
        log_winapi_fail("ReadFile");
        let _ = VirtualFree(asm_from_file, 0, MEM_RELEASE);
        return None;
    }

    let _ = CloseHandle(handle);
    Some((asm_from_file as *mut u8, payload_size))
}

/// Create a copy of the input data
pub unsafe fn create_input_data_copy(payload_base: *const u8, payload_size: u32) -> Option<*mut u8> {
    let memory_copy = VirtualAlloc(
        None,
        payload_size as usize,
        MEM_COMMIT,
        PAGE_READWRITE,
    );

    if memory_copy.is_null() {
        log_winapi_fail("VirtualAlloc");
        return None;
    }

    std::ptr::copy_nonoverlapping(
        payload_base,
        memory_copy as *mut u8,
        payload_size as usize,
    );

    Some(memory_copy as *mut u8)
}

/// Kill all child processes of the current process
pub unsafe fn kill_all_child_procs_of_current_proc() {
    log_program_message("Searching for any child processes that may have been spawned by the payload...");
    let current_proc_pid = GetCurrentProcessId();

    let Ok(h_process_snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
        log_program_fail("Process enumeration failed... Program will skip child process cleanup...");
        return;
    };

    let mut pe32 = PROCESSENTRY32 {
        dwSize: std::mem::size_of::<PROCESSENTRY32>() as u32,
        ..Default::default()
    };

    if Process32First(h_process_snap, &mut pe32).is_err() {
        let _ = CloseHandle(h_process_snap);
        log_program_fail("Process enumeration failed... Program will skip child process cleanup...");
        return;
    }

    loop {
        if pe32.th32ParentProcessID == current_proc_pid {
            log_program_message("Found child of our current process...");
            println!("    ---- CHILD PID = {}", pe32.th32ProcessID);

            if let Ok(handle_to_found_proc) = OpenProcess(PROCESS_TERMINATE, false, pe32.th32ProcessID) {
                if TerminateProcess(handle_to_found_proc, 0).is_err() {
                    log_winapi_fail("TerminateProcess");
                    log_program_fail("Failed to terminate the found child process... Continuing process enumeration...");
                }
                let _ = CloseHandle(handle_to_found_proc);
            } else {
                log_winapi_fail("OpenProcess");
                log_program_fail("Failed to open a handle to the found child process... Continuing process enumeration...");
            }
        }

        if Process32Next(h_process_snap, &mut pe32).is_err() {
            break;
        }
    }

    log_program_success("Finished process enumeration! All child processes of our current process were handled.");
    log_program_success("Child process cleanup successful!");
    let _ = CloseHandle(h_process_snap);
}

/// Replace executed assembly with INT3 instructions
pub fn replace_executed_asm_with_int3(
    payload_data: &mut [u8],
    payload_executed_asm: &IndexMap<u32, Vec<u8>>,
) {
    for (asm_offset, asm_bytes) in payload_executed_asm {
        let offset = *asm_offset as usize;
        let size = asm_bytes.len();

        if offset + size <= payload_data.len() {
            for i in 0..size {
                payload_data[offset + i] = ASM_INT3;
            }
        }
    }
}

/// Encrypt the map data with XOR encryption
pub fn encrypt_map_data(
    payload_executed_asm: &mut IndexMap<u32, Vec<u8>>,
    encryption_key: &[u8],
) {
    for (_, asm_bytes) in payload_executed_asm.iter_mut() {
        for (i, byte) in asm_bytes.iter_mut().enumerate() {
            if i < encryption_key.len() {
                *byte ^= encryption_key[i];
            }
        }
    }
}

/// Generate the output header file content
pub fn generate_out_file_data(
    payload_data: &[u8],
    payload_executed_asm: &IndexMap<u32, Vec<u8>>,
    encryption_key: &[u8],
) -> String {
    let mut result = String::new();

    // Add includes
    result.push_str("#pragma once\n");
    result.push_str("#include <unordered_map>\n");
    result.push_str("#include <iostream>\n\n");

    // Add payload array
    result.push_str("unsigned char payload[] = {");
    for (i, byte) in payload_data.iter().enumerate() {
        let hex_str = dword_to_hex_string(*byte as u32);
        let padded_hex = if hex_str.len() == 1 {
            format!("0{}", hex_str)
        } else {
            hex_str
        };

        result.push_str(&format!("0x{}", padded_hex));

        if i == payload_data.len() - 1 {
            result.push_str("};\n");
        } else {
            result.push(',');
        }
    }

    // Add payload size
    result.push_str("int payloadSize = sizeof(payload);\n");

    // Add map declaration
    result.push_str("std::unordered_map<DWORD, std::vector<unsigned char>> payloadExecutedAsm;\n");

    // Add encryption key declaration
    result.push_str("std::vector<unsigned char> encryptionKey;\n");

    // Add InitMap function
    result.push_str("void InitMap()\n{\n");

    // Generate encryption key initialization
    for byte in encryption_key {
        result.push_str(&format!("    encryptionKey.push_back('\\x{}');\n",
            dword_to_hex_string(*byte as u32)));
    }

    result.push_str("    std::vector<unsigned char> aux;\n");

    // Generate map data
    for (offset, asm_bytes) in payload_executed_asm {
        result.push_str("    aux.clear();\n");

        for byte in asm_bytes {
            result.push_str(&format!("    aux.push_back('\\x{}');\n",
                dword_to_hex_string(*byte as u32)));
        }

        result.push_str(&format!("    payloadExecutedAsm[{}] = aux;\n", offset));
    }

    result.push_str("}\n");

    result
}

/// Write the output file
pub unsafe fn write_output_file(
    payload_copy: &mut [u8],
    payload_executed_asm: &mut IndexMap<u32, Vec<u8>>,
    encryption_key: &[u8],
    out_file_path: &str,
) -> bool {
    let file_path_cstr = format!("{}\0", out_file_path);
    let out_file = CreateFileA(
        PCSTR(file_path_cstr.as_ptr()),
        (FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0),
        Default::default(),
        None,
        CREATE_NEW,
        FILE_ATTRIBUTE_NORMAL,
        None,
    );

    let Ok(out_file) = out_file else {
        log_winapi_fail("CreateFileA");
        return false;
    };

    if out_file == INVALID_HANDLE_VALUE {
        log_winapi_fail("CreateFileA");
        return false;
    }

    // Encrypt map data
    encrypt_map_data(payload_executed_asm, encryption_key);

    // Replace executed instructions with INT3
    replace_executed_asm_with_int3(payload_copy, payload_executed_asm);

    // Generate output file data
    let out_file_data = generate_out_file_data(payload_copy, payload_executed_asm, encryption_key);

    let mut bytes_written = 0;
    if WriteFile(
        out_file,
        Some(out_file_data.as_bytes()),
        Some(&mut bytes_written),
        None,
    ).is_err() {
        log_winapi_fail("WriteFile");
        let _ = CloseHandle(out_file);
        return false;
    }

    let _ = CloseHandle(out_file);
    true
}

/// Generate a random encryption key using Windows Crypto API
pub unsafe fn generate_encryption_key() -> Option<Vec<u8>> {
    use crate::veh::ASM_MAX_INSTRUCTION_LEN_X64;

    let mut h_prov = 0;
    if CryptAcquireContextA(
        &mut h_prov,
        PCSTR(null_mut()),
        PCSTR(null_mut()),
        PROV_RSA_AES,
        CRYPT_VERIFYCONTEXT,
    ).is_err() {
        log_winapi_fail("CryptAcquireContextA");
        return None;
    }

    let mut random_data = vec![0u8; ASM_MAX_INSTRUCTION_LEN_X64];
    if CryptGenRandom(h_prov, &mut random_data).is_err() {
        log_winapi_fail("CryptGenRandom");
        let _ = CryptReleaseContext(h_prov, 0);
        return None;
    }

    let _ = CryptReleaseContext(h_prov, 0);
    Some(random_data)
}
