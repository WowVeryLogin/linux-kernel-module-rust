use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::{env, fs};

const INCLUDED_TYPES: &[&str] = &[
    "file_system_type",
    "mode_t",
    "umode_t",
    "ctl_table",
    "ktime_t",
    "poll_table",
    "mutex",
    "spinlock_t",
    "wait_queue_head_t",
];
const INCLUDED_FUNCTIONS: &[&str] = &[
    "cdev_add",
    "cdev_init",
    "cdev_del",
    "register_filesystem",
    "unregister_filesystem",
    "krealloc",
    "kfree",
    "mount_nodev",
    "kill_litter_super",
    "register_sysctl",
    "unregister_sysctl_table",
    "access_ok",
    "_copy_to_user",
    "_copy_from_user",
    "alloc_chrdev_region",
    "unregister_chrdev_region",
    "wait_for_random_bytes",
    "get_random_bytes",
    "rng_is_initialized",
    "printk",
    "add_device_randomness",
    "ktime_get",
    "device_create",
    "device_destroy",
    "spin_lock_irq",
    "spin_unlock_irq",
    "spin_lock_init",
    "mutex_lock",
    "mutex_unlock",
    "mutex_init",
    "mutex_destroy",
    "init_waitqueue_head",
    "poll_wait",
    "wake_up_interruptible",
];
const INCLUDED_VARS: &[&str] = &[
    "EINVAL",
    "ENOMEM",
    "ESPIPE",
    "EFAULT",
    "EAGAIN",
    "__this_module",
    "FS_REQUIRES_DEV",
    "FS_BINARY_MOUNTDATA",
    "FS_HAS_SUBTYPE",
    "FS_USERNS_MOUNT",
    "FS_RENAME_DOES_D_MOVE",
    "BINDINGS_GFP_KERNEL",
    "KERN_INFO",
    "VERIFY_WRITE",
    "LINUX_VERSION_CODE",
    "SEEK_SET",
    "SEEK_CUR",
    "SEEK_END",
    "O_NONBLOCK",
    "POLLIN",
    "POLLRDNORM",
];
const OPAQUE_TYPES: &[&str] = &[
    // These need to be opaque because they're both packed and aligned, which rustc
    // doesn't support yet. See https://github.com/rust-lang/rust/issues/59154
    // and https://github.com/rust-lang/rust-bindgen/issues/1538
    "desc_struct",
    "xregs_state",
    "xsave_struct",
];
const CLANG_ARGS_BLACKLIST: [&'static str; 5] = [
    "-maccumulate-outgoing-args",
    "-mpreferred-stack-boundary=3",
    "-mindirect-branch=thunk-extern",
    "-mindirect-branch-register",
    "-fconserve-stack",
];

fn handle_kernel_version_cfg(bindings_path: &PathBuf) {
    let f = BufReader::new(fs::File::open(bindings_path).unwrap());
    let mut version = None;
    for line in f.lines() {
        let line = line.unwrap();
        if let Some(type_and_value) = line.split("pub const LINUX_VERSION_CODE").nth(1) {
            if let Some(value) = type_and_value.split('=').nth(1) {
                let raw_version = value.split(';').next().unwrap();
                version = Some(raw_version.trim().parse::<u64>().unwrap());
                break;
            }
        }
    }
    let version = version.expect("Couldn't find kernel version");
    let (major, minor) = match version.to_be_bytes() {
        [0, 0, 0, 0, 0, major, minor, _patch] => (major, minor),
        _ => panic!("unable to parse LINUX_VERSION_CODE {:x}", version),
    };

    if major >= 6 {
        panic!("Please update build.rs with the last 5.x version");
        // Change this block to major >= 7, copy the below block for
        // major >= 6, fill in unimplemented!() for major >= 5
    }
    if major >= 5 {
        for x in 0..=if major > 5 { unimplemented!() } else { minor } {
            println!("cargo:rustc-cfg=kernel_5_{}_0_or_greater", x);
        }
    }
    if major >= 4 {
        // We don't currently support anything older than 4.4
        for x in 1..=if major > 4 { 20 } else { minor } {
            println!("cargo:rustc-cfg=kernel_4_{}_0_or_greater", x);
        }
    }
    if major >= 3 {
        for x in 1..=if major > 3 { 19 } else { minor } {
            println!("cargo:rustc-cfg=kernel_3_{}_0_or_greater", x);
        }
    }
}

fn handle_kernel_symbols_cfg(symvers_path: &PathBuf) {
    let f = BufReader::new(fs::File::open(symvers_path).unwrap());
    for line in f.lines() {
        let line = line.unwrap();
        if let Some(symbol) = line.split_ascii_whitespace().nth(1) {
            if symbol == "setfl" {
                println!("cargo:rustc-cfg=kernel_aufs_setfl");
                break;
            }
        }
    }
}

// Takes the CFLAGS from the kernel Makefile and changes all the include paths to be absolute
// instead of relative.
fn prepare_cflags(cflags: &str, kernel_dir: &str) -> Vec<String> {
    let cflag_parts = shlex::split(&cflags).unwrap();
    let mut cflag_iter = cflag_parts.iter();
    let mut kernel_args = vec![];
    while let Some(arg) = cflag_iter.next() {
        if arg.starts_with("-I") && !arg.starts_with("-I/") {
            kernel_args.push(format!("-I{}/{}", kernel_dir, &arg[2..]));
        } else if arg == "-include" {
            kernel_args.push(arg.to_string());
            let include_path = cflag_iter.next().unwrap();
            if include_path.starts_with('/') {
                kernel_args.push(include_path.to_string());
            } else {
                kernel_args.push(format!("{}/{}", kernel_dir, include_path));
            }
        } else {
            kernel_args.push(arg.to_string());
        }
    }
    kernel_args
}

fn main() {
    println!("cargo:rerun-if-env-changed=CC");
    println!("cargo:rerun-if-env-changed=KDIR");
    println!("cargo:rerun-if-env-changed=c_flags");

    let kernel_dir = env::var("KDIR").expect("Must be invoked from kernel makefile");
    let kernel_cflags = env::var("c_flags").expect("Add 'export c_flags' to Kbuild");
    let kbuild_cflags_module =
        env::var("KBUILD_CFLAGS_MODULE").expect("Must be invoked from kernel makefile");

    let cflags = format!("{} {}", kernel_cflags, kbuild_cflags_module);
    let kernel_args = prepare_cflags(&cflags, &kernel_dir);

    let target = env::var("TARGET").unwrap();

    let mut builder = bindgen::Builder::default()
        .use_core()
        .ctypes_prefix("c_types")
        .derive_default(true)
        .size_t_is_usize(true)
        .rustfmt_bindings(true);

    builder = builder.clang_arg(format!("--target={}", target));
    for arg in kernel_args.iter() {
        if CLANG_ARGS_BLACKLIST.contains(&arg.as_str()) {
            continue;
        }
        if arg.as_str() == "-DOS_CENTOS" {
            println!("cargo:rustc-cfg=os_centos")
        }
        builder = builder.clang_arg(arg.clone());
    }

    println!("cargo:rerun-if-changed=src/bindings_helper.h");
    builder = builder.header("src/bindings_helper.h");

    for t in INCLUDED_TYPES {
        builder = builder.whitelist_type(t);
    }
    for f in INCLUDED_FUNCTIONS {
        builder = builder.whitelist_function(f);
    }
    for v in INCLUDED_VARS {
        builder = builder.whitelist_var(v);
    }
    for t in OPAQUE_TYPES {
        builder = builder.opaque_type(t);
    }
    let bindings = builder.generate().expect("Unable to generate bindings");

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");

    handle_kernel_version_cfg(&out_path.join("bindings.rs"));
    handle_kernel_symbols_cfg(&PathBuf::from(&kernel_dir).join("Module.symvers"));

    let mut builder = cc::Build::new();
    let compiler = env::var("CC").unwrap_or_else(|_| "clang".to_string());
    builder.compiler(&compiler);
    builder.target(&target);
    builder.warnings(false);
    println!("cargo:rerun-if-changed=src/helpers.c");
    builder.file("src/helpers.c");
    for arg in kernel_args.iter() {
        builder.flag(&arg);
    }
    if &compiler == "gcc" {
        builder.flag("-fno-pie");
    }
    builder.compile("helpers");
}
