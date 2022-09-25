use audiotags::{Tag};
use glob::glob;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::fs::File;
use std::io::BufReader;
use rodio::{Decoder, OutputStream, source::Source};
use shellexpand;

#[derive(Serialize, Deserialize)]
struct MyConfigs {
    folder: String,
    sync_key: String,
}
#[derive(Default, Debug, PartialEq)]
struct Book {
    title: String,
    files: Vec<String>,
    epub_file: String,
    time_stamp: u64,
    current_file:u32
}

/// `MyConfig` implements `Default`
impl ::std::default::Default for MyConfigs {
    fn default() -> Self {
        Self {
            folder: "~/AudioBooks".into(),
            sync_key: "0x837287328732888".into(),
        }
    }
}
fn glob_vec(pattern: &str) -> Vec<PathBuf> {
    glob(pattern).unwrap().map(|r| r.unwrap()).collect()
}
fn main() -> Result<(), confy::ConfyError> {
    let cfg: MyConfigs = confy::load("read_for_me", None)?;
    let expanded_folder=shellexpand::tilde(&cfg.folder);
    let m4a_files_pattern = expanded_folder.clone() + "/**/*.m4a";
    let mp3_files_pattern = expanded_folder.clone() + "/**/*.mp3";
    println!("path is {:?}", m4a_files_pattern);
    println!("path is {:?}", mp3_files_pattern);

    let mut m4a_files = glob_vec(&m4a_files_pattern.to_string());
    let mp3_files = glob_vec(&mp3_files_pattern.to_string());
    m4a_files.extend(mp3_files);
    let all_files=m4a_files;
    println!("number of files {:?}", all_files.len());

    let mut books: HashMap<String, Book> = HashMap::new();

    for file in all_files{
        println!("m4a {:?}", file.as_os_str());
        let tag = Tag::new().read_from_path(file.as_os_str());
        match tag {
            Ok(audiotags) => {
                let title = audiotags.album_title().unwrap().to_string();
                if books.contains_key(&title) == false {
                    let mut files: Vec<String> = Vec::new();

                    files.push(file.to_str().unwrap().to_string());
                    let book_ = Book {
                        title: audiotags.album_title().unwrap().to_string(),
                        files: files,
                        epub_file: "".to_string(),
                        time_stamp:0,
                        current_file:0
                    };
                    books.insert(title, book_);
                } else {
                    let book = books.get_mut(&title);
                    match book {
                        Some(book_) => {
                            book_.files.push(file.to_str().unwrap().to_string());
                        }
                        None => println!("Error Not found"),
                    }
                }
                println!("{:?}", audiotags.album_title())
            }
            Err(_) => println!("Couldnot handle this file problem with metadata {:?}", file),
        }
        // let metadata = fs::metadata(m4.as_os_str()).expect("Problem getting meta data");
    }
    println!("{:?}", books.keys());
    println!("{:?}",books["Dorothy & the Wizard in Oz"].title);
    println!("{:?}",books["Dorothy & the Wizard in Oz"].files);
    println!("{:?}",books["Dorothy & the Wizard in Oz"].epub_file);
    println!("{:?}",books["Dorothy & the Wizard in Oz"].time_stamp);
    let atlas_shrugged=books["Dorothy & the Wizard in Oz"].files[0].clone();
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let file=BufReader::new(File::open(atlas_shrugged).unwrap());
    let source = Decoder::new(file).unwrap();
    stream_handle.play_raw(source.convert_samples());
    std::thread::sleep(std::time::Duration::from_secs(40));
    Ok(())
}
