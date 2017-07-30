extern crate download_sdl2;

use std::env;
use std::fs;

#[test]
fn download_test() {
    let test_dir = "test_manfest_dir";
    env::set_var("CARGO_MANIFEST_DIR", test_dir);
    env::set_var("TARGET", "stable-x86_64-pc-windows-msvc");
    fs::create_dir_all(test_dir).expect("create test dir");
    assert!(download_sdl2::download().is_ok());
    fs::remove_dir_all(test_dir).expect("delete test dir");
}
