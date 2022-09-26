use audiotags::{Tag};
use glob::glob;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use shellexpand;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::errors::Error;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
mod output;
use symphonia::core::formats::{Cue, FormatOptions, FormatReader, SeekMode, SeekTo, Track};
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
    let src = std::fs::File::open(&atlas_shrugged).expect("failed to open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = FormatOptions{enable_gapless:true,prebuild_seek_index:false,seek_index_fill_rate:20};
    // Probe the media source.
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)
                                                .expect("unsupported format");
    let mut format = probed.format;

    // Find the first audio track with a known (decodeable) codec.
    let track = format.tracks()
                    .iter()
                    .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                    .expect("no supported audio tracks");

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)
                                                    .expect("unsupported codec");

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;
    let mut audio_output=None;
    let mut first_time=true;
    // let &mut audio_output_=audio_output;
    let mut track_info = PlayTrackOptions { seek_ts:0, track_id:0 };
    // let audio_output=&audio_output_;
    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(err) => {
                // A unrecoverable error occured, halt decoding.
                if err.to_string().contains("end of stream"){
                    println!("end of stream {}",err);
                    break;
                }else{
                    panic!("{}", err);

                }
            }
        };

        // Consume any new metadata that has been read since the last packet.
        while !format.metadata().is_latest() {
            // Pop the old head of the metadata queue.
            format.metadata().pop();

            // Consume the new metadata at the head of the metadata queue.
        }

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                if first_time{
                    // If the audio output is not open, try to open it.
                    // Get the audio buffer specification. This is a description of the decoded
                    // audio buffer's sample format and sample rate.
                    let spec = *decoded.spec();

                    // Get the capacity of the decoded buffer. Note that this is capacity, not
                    // length! The capacity of the decoded buffer is constant for the life of the
                    // decoder, but the length is not.
                    let duration = decoded.capacity() as u64;

                    // Try to open the audio output.
                    audio_output=Some(output::try_open(spec, duration).unwrap());
                    first_time=false;
                }
                else{
                    if packet.ts() >= track_info.seek_ts {
                        // if let Some(audio_output) = audio_output {
                        //     audio_output.write(decoded).unwrap()
                        // }
                        audio_output.unwrap().write(decoded);
                    }
                }

                // Consume the decoded audio samples (see below).
                // match audio_output{
                //     Ok(audio_output_)=> audio_output_.wr
                // }

            }
            Err(Error::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                continue;
            }
            Err(Error::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                continue;
            }
            Err(err) => {
                // An unrecoverable error occured, halt decoding.
                panic!("{}", err);
            }
        }
    }
    // if let Some(audio_output) = audio_output.as_mut() {
    //     audio_output.flush()
    // }
    Ok(())
}

struct PlayTrackOptions {
    track_id: u32,
    seek_ts: u64,
}

