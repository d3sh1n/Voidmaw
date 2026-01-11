mod utils;
mod veh;
mod core;

use clap::Parser;
use indexmap::IndexMap;
use windows::Win32::System::Diagnostics::Debug::AddVectoredExceptionHandler;
use windows::Win32::System::Memory::{VirtualProtect, VirtualFree, MEM_RELEASE, PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_GUARD};
use windows::Win32::System::Threading::{CreateThread, TerminateThread, THREAD_CREATION_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxA, MESSAGEBOX_STYLE};
use windows::core::s;
use std::ptr::null_mut;

use utils::*;
use veh::*;
use core::*;

#[derive(Parser, Debug)]
#[command(name = "Dismantle")]
#[command(about = "Payload instruction recorder for Voidmaw", long_about = None)]
struct Args {
    /// Path to the initial payload file
    #[arg(short = 'p', long, value_name = "FILE")]
    payload: String,

    /// Path where the output header file will be saved
    #[arg(short = 'o', long, value_name = "FILE")]
    output: String,

    /// Arguments to pass to the payload (optional)
    #[arg(short = 'a', long, value_name = "ARGS", default_value = "")]
    args: String,
}

fn main() -> std::process::ExitCode {
    let args = Args::parse();

    unsafe {
        // Validate program arguments
        log_program_message("Validating program arguments...");
        let Some(input_file_handle) = check_program_args(&args.payload) else {
            log_program_fail("Argument validation failed... Program will now terminate...");
            return std::process::ExitCode::FAILURE;
        };

        // Generate encryption key
        log_program_message("Generating the encryption key...");
        let Some(encryption_key) = generate_encryption_key() else {
            log_program_fail("Failed to generate encryption key... Program will now terminate...");
            return std::process::ExitCode::FAILURE;
        };

        // Read the input file
        log_program_message("Reading the raw data from the input file...");
        let Some((payload_base, payload_size)) = read_input_file(input_file_handle) else {
            log_program_fail("Failed to read the raw data from the input file... Program will now terminate...");
            return std::process::ExitCode::FAILURE;
        };

        // Create a backup copy
        log_program_message("Creating a backup of the input file data...");
        let Some(payload_copy) = create_input_data_copy(payload_base, payload_size) else {
            log_program_fail("Failed to create copy of the input data... Program will now terminate...");
            let _ = VirtualFree(payload_base as *const _, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        };

        // Create the map for recording executed instructions
        let mut payload_executed_asm: IndexMap<u32, Vec<u8>> = IndexMap::new();
        let payload_executed_asm_ptr = &mut payload_executed_asm as *mut IndexMap<u32, Vec<u8>>;

        // Initialize VEH globals
        init_veh_globals(payload_executed_asm_ptr, payload_base, payload_size);

        // Install the VEH for recording
        log_program_message("Installing page guard VEH...");
        let page_guard_veh = AddVectoredExceptionHandler(1, Some(veh_page_guard));
        if page_guard_veh.is_err() || page_guard_veh.as_ref().unwrap().is_invalid() {
            log_winapi_fail("AddVectoredExceptionHandler");
            let _ = VirtualFree(payload_copy as *const _, 0, MEM_RELEASE);
            let _ = VirtualFree(payload_base as *const _, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        }

        // Set PAGE_GUARD protection
        log_program_message("Setting up page guard protection on payload memory...");
        let mut old_protect = PAGE_PROTECTION_FLAGS(0);
        if VirtualProtect(
            payload_base as *const _,
            payload_size as usize,
            PAGE_EXECUTE_READWRITE | PAGE_GUARD,
            &mut old_protect,
        ).is_err() {
            log_winapi_fail("VirtualProtect");
            let _ = VirtualFree(payload_copy as *const _, 0, MEM_RELEASE);
            let _ = VirtualFree(payload_base as *const _, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        }

        // Note: Command line argument modification is complex and OS-specific
        // For now, we skip this feature in the Rust version
        // Original C++ code modifies PEB which requires more intricate unsafe code

        // Create thread on the payload
        log_program_message("Creating new thread on the payload...");
        let run_payload = CreateThread(
            None,
            0,
            Some(std::mem::transmute(payload_base)),
            Some(null_mut()),
            THREAD_CREATION_FLAGS(0),
            None,
        );

        let Ok(run_payload) = run_payload else {
            log_winapi_fail("CreateThread");
            let _ = VirtualFree(payload_copy as *const _, 0, MEM_RELEASE);
            let _ = VirtualFree(payload_base as *const _, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        };

        // Wait for user to stop recording
        log_program_message("...WAITING-FOR-STOP-SIGNAL...");

        let message = s!("Press \"OK\" button to stop the recording of the payload execution and start generating the final output file.");
        let title = s!("WAITING USER INPUT...");
        MessageBoxA(None, message, title, MESSAGEBOX_STYLE(0));

        // Terminate the payload thread
        let _ = TerminateThread(run_payload, 0);

        // Kill any child processes
        kill_all_child_procs_of_current_proc();

        // Generate and write the output file
        log_program_message(&format!("Writing the generated header file to the output path of {}", args.output));

        let payload_copy_slice = std::slice::from_raw_parts_mut(
            payload_copy,
            payload_size as usize,
        );

        if !write_output_file(
            payload_copy_slice,
            &mut payload_executed_asm,
            &encryption_key,
            &args.output,
        ) {
            log_program_fail("Failed to write the output file... Program will now terminate...");
            let _ = VirtualFree(payload_copy as *const _, 0, MEM_RELEASE);
            let _ = VirtualFree(payload_base as *const _, 0, MEM_RELEASE);
            return std::process::ExitCode::FAILURE;
        }

        log_program_success("Header file successfully written on disk!");
        log_program_success("Program executed successfully!");

        // Cleanup
        let _ = VirtualFree(payload_copy as *const _, 0, MEM_RELEASE);
        let _ = VirtualFree(payload_base as *const _, 0, MEM_RELEASE);

        std::process::ExitCode::SUCCESS
    }
}
