extern crate flate2;
extern crate glob;
extern crate hyper;
extern crate tar;
extern crate zip;

use flate2::read::GzDecoder;
use glob::Pattern;
use hyper::Url;
use hyper::client::Client;
use std::env;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::fs::{create_dir_all, DirBuilder, File, OpenOptions};
use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use tar::Archive;
use zip::ZipArchive;
use zip::result::ZipResult;

const SDL2_VERSION: &'static str = "2.0.6";
const SDL2_IMAGE_VERSION: &'static str = "2.0.1";
const SDL2_TTF_VERSION: &'static str = "2.0.14";

fn zip_filename_to_target_filename(
    zip_filename: &Path,
    target_path: &Path,
) -> Option<PathBuf> {
    let bit64 = Pattern::new("**/x64/*").unwrap();
    let bit32 = Pattern::new("**/x86/*").unwrap();
    zip_filename.file_name().and_then(|filename| {
        let new_path = if Pattern::new("*/lib/*/*.dll")
            .unwrap()
            .matches_path(zip_filename)
        {
            Some(target_path.join("dll"))
        } else if Pattern::new("*/lib/*/*.lib")
            .unwrap()
            .matches_path(zip_filename)
        {
            Some(target_path.join("lib"))
        } else {
            None
        };

        new_path
            .and_then(|path| if bit64.matches_path(zip_filename) {
                Some(path.join("64"))
            } else if bit32.matches_path(zip_filename) {
                Some(path.join("32"))
            } else {
                None
            })
            .map(|path| path.join(filename))
    })
}

fn ungzip_file(zipfile: &Path, target_path: &Path) -> Result<(), Box<Error>> {
    let f = File::open(zipfile).expect("file should open");
    let buf_reader = BufReader::new(f);
    let decoder = GzDecoder::new(buf_reader).expect("file should gzdecode");
    let mut archive = Archive::new(decoder);
    for entry in archive.entries()? {
        let real_entry = entry?;
        let filepath = real_entry.path()?.clone();
        // println!("filepath: {:?}", filepath);

        if let Some(target_filename) =
            zip_filename_to_target_filename(&filepath, target_path)
        {
            // FIXME: Cope the mingw dll/libs over
            // println!("Target filename: {:?}", target_filename);
        }
    }

    Ok(())
}

fn unzip_file(zipfile: &Path, target_path: &Path) -> ZipResult<()> {
    let f = File::open(zipfile)?;
    let reader = BufReader::new(f);

    let mut zip = ZipArchive::new(reader).expect("open zip file");
    let mut files_to_extract = Vec::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        let filepath = Path::new(file.name());
        // println!("Filename: {:?}", filepath);
        if let Some(target_filename) =
            zip_filename_to_target_filename(filepath, target_path)
        {
            // println!("Target filename: {:?}", target_filename);
            files_to_extract.push((target_filename, i));
        }
    }

    for (target_filename, i) in files_to_extract {
        let file = zip.by_index(i)?;

        if let Some(parent_dir) = target_filename.clone().parent() {
            create_dir_all(parent_dir)?;
            let mut outfile = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target_filename.clone())?;
            let mut buf_reader = BufReader::new(file);
            io::copy(&mut buf_reader, &mut outfile)?;
        }
    }

    Ok(())
}

#[derive(Debug)]
struct PathError {}

impl Error for PathError {
    fn description(&self) -> &str {
        "Error parsing path segments"
    }
}

impl Display for PathError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "PathError")
    }
}

fn download_file(download_dir: &Path, url: Url) -> Result<PathBuf, Box<Error>> {
    let url_filename = url.path_segments()
        .ok_or(PathError {})?
        .last()
        .ok_or(PathError {})?;
    let filename = download_dir.join(url_filename);
    if !filename.exists() {
        // println!("going to try downloading to: {:?}", filename);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(filename.clone())?;
        let client = Client::new();
        let mut res = client.get(url.clone()).send()?;
        io::copy(&mut res, &mut file)?;
    }
    Ok(filename)
}

fn fetch_windows_libraries(manifest_dir: &Path) -> Result<(), Box<Error>> {
    let download_dir = manifest_dir.join("target").join("downloads");
    DirBuilder::new()
        .recursive(true)
        .create(&download_dir)
        .expect("create target/downloads dir");

    let downloads = [
        (
            format!(
                "http://www.libsdl.org/release/SDL2-devel-{}-VC.zip",
                SDL2_VERSION
            ),
            "sdl2 VC",
            "msvc",
        ),
        (
            format!(
                "http://www.libsdl.org/release/SDL2-devel-{}-mingw.tar.gz",
                SDL2_VERSION
            ),
            "sdl2 mingw",
            "gnu-mingw",
        ),
        (
            format!(
                "http://www.libsdl.org/projects/SDL_image/release/SDL2_image-devel-{}-VC.zip",
                SDL2_IMAGE_VERSION
            ),
            "sdl2_image VC",
            "msvc",
        ),
        (
            format!(
                "http://www.libsdl.org/projects/SDL_image/release/SDL2_image-devel-{}-mingw.tar.gz",
                SDL2_IMAGE_VERSION
            ),
            "sdl2_image mingw",
            "gnu-mingw",
        ),
        (
            format!(
                "http://www.libsdl.org/projects/SDL_ttf/release/SDL2_ttf-devel-{}-VC.zip",
                SDL2_TTF_VERSION
            ),
            "sdl2_ttf VC",
            "msvc",
        ),
        (
            format!(
                "http://www.libsdl.org/projects/SDL_ttf/release/SDL2_ttf-devel-{}-mingw.tar.gz",
                SDL2_TTF_VERSION
            ),
            "sdl2_ttf mingw",
            "gnu-mingw",
        ),
    ];

    for &(ref url, label, dir) in downloads.into_iter() {
        let expect_str = format!("valid {} url", label);
        let zipfile = download_file(
            download_dir.as_path(),
            Url::parse(&url).expect(&expect_str),
        )?;
        let target_dir = manifest_dir.join(dir);
        zipfile.extension().map(|ext| {
            // println!("ext: {:?}", ext);
            if ext == OsStr::new("zip") {
                let _ = unzip_file(&zipfile, &target_dir);
            } else if ext == OsStr::new("gz") {
                let _ = ungzip_file(&zipfile, &target_dir);
            }
        });
    }
    Ok(())
}

pub fn download() -> Result<(), Box<Error>> {
    let target = env::var("TARGET").unwrap();
    if target.contains("pc-windows") {
        let manifest_dir =
            PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

        let _ = fetch_windows_libraries(manifest_dir.as_path());

        let mut lib_dir = manifest_dir.clone();
        let mut dll_dir = manifest_dir.clone();
        if target.contains("msvc") {
            lib_dir.push("msvc");
            dll_dir.push("msvc");
        } else {
            lib_dir.push("gnu-mingw");
            dll_dir.push("gnu-mingw");
        }
        lib_dir.push("lib");
        dll_dir.push("dll");

        if target.contains("x86_64") {
            lib_dir.push("64");
            dll_dir.push("64");
        } else {
            lib_dir.push("32");
            dll_dir.push("32");
        }
        println!("cargo:rustc-link-search=all={}", lib_dir.display());
        let dll_dir_msg = format!("Can't read DDL dir: {:?}", dll_dir);
        for entry in std::fs::read_dir(dll_dir).expect(&dll_dir_msg) {
            let entry_path = entry.expect("Invalid fs entry").path();
            let file_name_result = entry_path.file_name();
            let mut new_file_path = manifest_dir.clone();
            if let Some(file_name) = file_name_result {
                let file_name = file_name.to_str().unwrap();
                if file_name.ends_with(".dll") {
                    new_file_path.push(file_name);
                    std::fs::copy(&entry_path, new_file_path.as_path())
                        .expect("Can't copy from DLL dir");
                }
            }
        }
    }

    Ok(())
}
