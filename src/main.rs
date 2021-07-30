use std::sync::Mutex;

#[macro_use]
extern crate lazy_static;

extern crate gdk;
extern crate gtk;

use gtk::prelude::*;

extern crate capstone;
use capstone::prelude::*;

extern crate keystone;
use keystone::{Arch as KeystoneArch, Keystone};

extern crate hex;

#[derive(Debug, Clone, Copy)]
struct Config {
    arch: MyArch,
    mode: MyMode,
}

#[derive(Debug, Clone, Copy)]
enum MyArch {
    X86,
    Arm,
}

#[derive(Debug, Clone, Copy)]
enum MyMode {
    Bits32,
    Bits64,
}

impl Config {
    fn new() -> Config {
        Config {
            arch: MyArch::X86,
            mode: MyMode::Bits64,
        }
    }

    fn set_arch(&mut self, new_arch: MyArch) {
        self.arch = new_arch;
    }

    fn set_mode(&mut self, new_mode: MyMode) {
        self.mode = new_mode;
    }

    fn get(&self) -> (MyArch, MyMode) {
        (self.arch, self.mode)
    }
}

lazy_static! {
    static ref CONFIG: Mutex<Config> = {
        let c = Config::new();
        Mutex::new(c)
    };
}

fn main() {
    if gtk::init().is_err() {
        println!("Failed to initialize GTK.");
        return;
    }

    let glade_src = include_str!("guidra.glade");
    let builder = gtk::Builder::from_string(glade_src);

    // Define x86_64 as default architecture
    let label: gtk::Label = builder.object("label_mode").unwrap();
    label.set_label("x64");
    CONFIG.lock().unwrap().set_arch(MyArch::X86);
    CONFIG.lock().unwrap().set_mode(MyMode::Bits64);

    register_buttons(&builder);

    gtk::main();
}

/// Register buttons related to configuration. They can be used to change the
/// configuration of Capstone and Keystone engines.
fn register_config_buttons(builder: &gtk::Builder) {
    let button_x86: gtk::Button = builder.object("button_x86").unwrap();
    let b = builder.clone();

    button_x86.connect_clicked(move |_| {
        let label: gtk::Label = b.object("label_mode").unwrap();
        label.set_label("x86");
        CONFIG.lock().unwrap().set_arch(MyArch::X86);
        CONFIG.lock().unwrap().set_mode(MyMode::Bits32);
    });

    let button_x64: gtk::Button = builder.object("button_x64").unwrap();
    let b = builder.clone();
    button_x64.connect_clicked(move |_| {
        let label: gtk::Label = b.object("label_mode").unwrap();
        label.set_label("x64");
        CONFIG.lock().unwrap().set_arch(MyArch::X86);
        CONFIG.lock().unwrap().set_mode(MyMode::Bits64);
    });

    let button_arm: gtk::Button = builder.object("button_arm").unwrap();
    let b = builder.clone();
    button_arm.connect_clicked(move |_| {
        let label: gtk::Label = b.object("label_mode").unwrap();
        label.set_label("ARM 32 bits little endian");
        CONFIG.lock().unwrap().set_arch(MyArch::Arm);
        CONFIG.lock().unwrap().set_mode(MyMode::Bits32);
    });

    let button_aarch64: gtk::Button = builder.object("button_aarch64").unwrap();
    let b = builder.clone();
    button_aarch64.connect_clicked(move |_| {
        let label: gtk::Label = b.object("label_mode").unwrap();
        label.set_label("ARM 64 bits little endian");
        CONFIG.lock().unwrap().set_arch(MyArch::Arm);
        CONFIG.lock().unwrap().set_mode(MyMode::Bits64);
    });
}

/// Register buttons of the UI.
fn register_buttons(builder: &gtk::Builder) {
    register_action_buttons(builder);
    register_config_buttons(builder);
}

/// Register buttons related to actions. They can be used to assemble or
/// disassemble user provided data.
fn register_action_buttons(builder: &gtk::Builder) {
    let window: gtk::Window = builder.object("applicationWindow").unwrap();
    let button_disas: gtk::Button = builder.object("button_disas").unwrap();
    let button_ass: gtk::Button = builder.object("button_ass").unwrap();

    window.connect_delete_event(|_, _| {
        // Stop the main loop.
        gtk::main_quit();
        // Let the default handler destroy the window.
        Inhibit(false)
    });

    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(800, 800);

    let text_view_asm: gtk::TextView = builder
        .object("text_view_asm")
        .expect("Couldn't get text_view_asm");
    let text_view_hex: gtk::TextView = builder
        .object("text_view_hex")
        .expect("Couldn't get text_view_hex");

    let th = text_view_hex.clone();
    let ta = text_view_asm.clone();

    let b = builder.clone();
    // Clicked button Disassemble
    button_disas.connect_clicked(move |_| {
        let buffer = th.buffer().unwrap();
        let content = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);

        // Remove new lines in order to support multi line hex strings
        let stripped_content = content.unwrap().to_string().replace("\n", "");

        // Read opcodes from user. Format is a hex string like 55488b05b8130000e9149e08004531e4
        let opcodes: Vec<u8> = match hex::decode(stripped_content) {
            Ok(o) => o,
            Err(_) => {
                set_label_status("Invalid hex string", &b);
                return;
            }
        };

        // Disassemble code and print it out in dedicated buffer
        let disassembly = disassemble(&opcodes[..], &b);
        ta.buffer().unwrap().set_text(&disassembly);
    });

    let b = builder.clone();
    // Clicked button Assemble
    button_ass.connect_clicked(move |_| {
        let buffer = text_view_asm.buffer().unwrap();
        let content = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);

        let asm = assemble(content.unwrap().to_string(), &b);

        text_view_hex.buffer().unwrap().set_text(&asm);
    });

    window.show_all();
}

/// Disassemble thanks to Capstone engine.
fn disassemble(code: &[u8], builder: &gtk::Builder) -> String {
    let config = CONFIG.lock().unwrap();
    let cs = match config.get() {
        (MyArch::Arm, MyMode::Bits32) => Capstone::new()
            .arm()
            .mode(arch::arm::ArchMode::Arm)
            .detail(true)
            .build()
            .unwrap(),
        (MyArch::Arm, MyMode::Bits64) => Capstone::new()
            .arm64()
            .mode(arch::arm64::ArchMode::Arm)
            .detail(true)
            .build()
            .unwrap(),
        (MyArch::X86, MyMode::Bits32) => Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode32)
            .detail(true)
            .build()
            .unwrap(),
        (MyArch::X86, MyMode::Bits64) => Capstone::new()
            .x86()
            .mode(arch::x86::ArchMode::Mode64)
            .detail(true)
            .build()
            .unwrap(),
    };

    let start_addr = 0;

    // Disassemble the code
    let insns = match cs.disasm_all(code, start_addr) {
        Ok(i) => i,
        Err(e) => {
            set_label_status(&e.to_string(), builder);
            return String::new();
        }
    };

    set_label_status("Performed Disassemble successfully", builder);

    let mut res = String::new();
    for i in insns.iter() {
        res += &i.to_string();
        res += "\n";
    }
    res
}

/// Disassemble thanks to Keystone engine.
fn assemble(code: String, builder: &gtk::Builder) -> String {
    let config = CONFIG.lock().unwrap();
    let engine = match config.get() {
        (MyArch::Arm, MyMode::Bits32) => {
            Keystone::new(KeystoneArch::ARM, keystone::Mode::ARM).unwrap()
        }
        (MyArch::Arm, MyMode::Bits64) => {
            Keystone::new(KeystoneArch::ARM64, keystone::Mode::LITTLE_ENDIAN).unwrap()
        }
        (MyArch::X86, MyMode::Bits32) => {
            Keystone::new(KeystoneArch::X86, keystone::Mode::MODE_32).unwrap()
        }
        (MyArch::X86, MyMode::Bits64) => {
            Keystone::new(KeystoneArch::X86, keystone::Mode::MODE_64).unwrap()
        }
    };

    let result = match engine.asm(code, 0x1000) {
        Ok(r) => r,
        Err(e) => {
            set_label_status(&e.to_string(), builder);
            return String::new();
        }
    };

    set_label_status("Performed Assemble successfully", builder);

    hex::encode(result.bytes)
}

#[allow(dead_code)]
/// Open a dialog box with a custom `message`
fn message_box(message: &str) {
    gtk::MessageDialog::new(
        None::<&gtk::Window>,
        gtk::DialogFlags::empty(),
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        message,
    )
    .run();
}

fn set_label_status(message: &str, builder: &gtk::Builder) {
    let label: gtk::Label = builder.object("label_status").unwrap();
    label.set_label(message);
}
