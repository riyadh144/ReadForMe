use audiotags::Tag;
use glob::glob;
use serde_derive::{Deserialize, Serialize};
use shellexpand;
use std::collections::HashMap;
use std::collections::hash_map::Keys;
use std::iter::Enumerate;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, channel};
use std::thread;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::{Time, TimeStamp};
mod output;
use log::{error, info, warn};
use std::thread::sleep;
use std::time::Duration;
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
    current_file: u32,
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
   
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "My egui App",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    );

    Ok(())
}

use eframe::egui;

struct MyApp {
    books:HashMap<String, Book>,
    selected_book:String,

    is_book_playing:bool,
    tx:Option<Sender<String>>,

}
impl Default for MyApp {
    fn default() -> Self {
        Self {
            books:get_books(),
            selected_book:String::default(),
            is_book_playing:false,
            tx:None,

        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            for book in &self.books{
                if ui.button(book.0).clicked(){
                    self.selected_book=book.0.clone();
                }
            }
            ui.heading("README");
            if ui.button("Play").clicked(){
                let txrx=mpsc::channel();
                (self.tx)=(Some(txrx.0));
                let rx=Some(txrx.1);
                let files=&self.books[&self.selected_book].files;
                let file=files[0].clone();
                thread::spawn(move || play_file(file,rx.expect("Bad RX")));
                self.is_book_playing=true;
            }
            // ui.horizontal(|ui| {
            //     ui.label("Your name: ");
            //     ui.text_edit_singleline(&mut self.name);
            // });
            // ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            // if ui.button("Click each year").clicked() {
            //     self.age += 1;
            // }
            // ui.label(format!("Hello '{}', age {}", self.name, self.age));
        });
    }
}

fn get_books()->HashMap<String, Book>{
    let cfg: MyConfigs = confy::load("read_for_me", None).expect("Failed to read");
    let expanded_folder = shellexpand::tilde(&cfg.folder);
    let m4a_files_pattern = expanded_folder.clone() + "/**/*.m4a";
    let mp3_files_pattern = expanded_folder.clone() + "/**/*.mp3";
    println!("path is {:?}", m4a_files_pattern);
    println!("path is {:?}", mp3_files_pattern);

    let mut m4a_files = glob_vec(&m4a_files_pattern.to_string());
    let mp3_files = glob_vec(&mp3_files_pattern.to_string());
    m4a_files.extend(mp3_files);
    let all_files = m4a_files;
    println!("number of files {:?}", all_files.len());

    let mut books: HashMap<String, Book> = HashMap::new();

    for file in all_files {
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
                        time_stamp: 0,
                        current_file: 0,
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
    return books
}
fn number_elements_keys<T>(list:Keys<String,T>)->HashMap<String,&String>{
    let mut output_:HashMap<String,&String>=HashMap::new();
    let mut index=0;
    for item in list{
        // output_=format!("{output_} \n {index}:{item}").to_string();
        output_.insert(index.to_string(), item);
        index=index+1;
    }
    output_
}
fn number_elements_vec(list:Vec<String>)->HashMap<String,String>{
    let mut output_:HashMap<String,String>=HashMap::new();
    let mut index=0;
    for item in list{
        // output_=format!("{output_} \n {index}:{item}").to_string();
        output_.insert(index.to_string(), item);
        index=index+1;
    }
    output_
}
struct PlayTrackOptions {
    track_id: u32,
    seek_ts: u64,
}

fn play_file(file: String, rx: Receiver<String>) {
    let src = std::fs::File::open(&file).expect("failed to open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = FormatOptions {
        enable_gapless: true,
        prebuild_seek_index: false,
        seek_index_fill_rate: 20,
    };
    // Probe the media source.
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .expect("unsupported format");
    let mut format = probed.format;

    // Find the first audio track with a known (decodeable) codec.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .expect("unsupported codec");

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;
    // let mut audio_output=None;
    let mut first_time = true;
    // let &mut audio_output_=audio_output;
    let mut track_info = PlayTrackOptions {
        seek_ts: 0,
        track_id: 0,
    };

    play_track(&mut format, track_info, &dec_opts, true, &rx);
}

fn play_track(
    reader: &mut Box<dyn FormatReader>,
    play_opts: PlayTrackOptions,
    decode_opts: &DecoderOptions,
    no_progress: bool,
    rx: &Receiver<String>,
) {
    let mut audio_output_ = None;
    let audio_output = &mut audio_output_;
    // Get the selected track using the track ID.
    let track = match reader
        .tracks()
        .iter()
        .find(|track| track.id == play_opts.track_id)
    {
        Some(track) => track,
        _ => panic!("Good"),
    };
    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, decode_opts)
        .unwrap();

    // Get the selected track's timebase and duration.
    let tb = track.codec_params.time_base.unwrap();
    let dur = track
        .codec_params
        .n_frames
        .map(|frames| track.codec_params.start_ts + frames)
        .unwrap();
    let dur_secs = tb.calc_time(dur).seconds;
    println!("duration {:?}", dur_secs);
    // Decode and play the packets belonging to the selected track.
    loop {
        // Get the next packet from the format reader.
        let packet = match reader.next_packet() {
            Ok(packet) => packet,
            Err(err) => break,
        };

        // If the packet does not belong to the selected track, skip it.
        if packet.track_id() != play_opts.track_id {
            continue;
        }
        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(decoded) => {
                // If the audio output is not open, try to open it.
                if audio_output.is_none() {
                    // Get the audio buffer specification. This is a description of the decoded
                    // audio buffer's sample format and sample rate.
                    let spec = *decoded.spec();

                    // Get the capacity of the decoded buffer. Note that this is capacity, not
                    // length! The capacity of the decoded buffer is constant for the life of the
                    // decoder, but the length is not.
                    let duration = decoded.capacity() as u64;

                    // Try to open the audio output.
                    audio_output.replace(output::try_open(spec, duration).unwrap());
                } else {
                    // TODO: Check the audio spec. and duration hasn't changed.
                }
                let mut seek = |time_to_seek: i64| {
                    let seek_mode = SeekMode::Accurate;
                    let current_time = tb.calc_time(packet.ts()).seconds;
                    let mut seeked_time = 0;
                    if time_to_seek >= 0 {
                        seeked_time = current_time + (time_to_seek as u64);
                    } else if (time_to_seek < 0 && current_time > time_to_seek.abs() as u64) {
                        seeked_time = current_time - time_to_seek.abs() as u64
                    }

                    let packet_time_stamp = tb.calc_timestamp(Time {
                        seconds: seeked_time,
                        frac: 0.0,
                    });
                    if packet_time_stamp < dur {
                        // packet.trim_end10
                        let seek_to = SeekTo::Time {
                            time: Time {
                                seconds: seeked_time,
                                frac: 0.0,
                            },
                            track_id: Some(packet.track_id()),
                        };
                        let seeked_to = reader.seek(seek_mode, seek_to).expect("couldn't seek");
                        println!(
                            "Current Time is {:?} current track {:?}",
                            tb.calc_time(seeked_to.actual_ts),
                            seeked_to.track_id
                        )
                    } else {
                        // packet.trim_end10
                        let time_into_new_track = packet_time_stamp - dur;
                        let seek_to = SeekTo::TimeStamp {
                            ts: time_into_new_track,
                            track_id: packet.track_id() + 1,
                        };
                        let seeked_to = reader.seek(seek_mode, seek_to).expect("couldn't seek");
                        println!(
                            "Current Time is {:?} current track {:?}",
                            tb.calc_time(seeked_to.actual_ts),
                            seeked_to.track_id
                        )
                    }
                };
                let received = rx.try_recv();
                match received {
                    Ok(command) => {
                        // println!("command received {:?}",command);
                        match command.as_str() {
                            "seek10\n" => {
                                seek(10);
                            }
                            "seek30\n" => {
                                seek(30);
                            }
                            "seek60\n" => {
                                seek(60);
                            }
                            "seek5min\n" => {
                                seek(300);
                            }
                            "seek-10\n" => {
                                seek(-10);
                            }
                            "seek-30\n" => {
                                seek(-30);
                            }
                            "seek-60\n" => {
                                seek(-60);
                            }
                            "seek-5min\n" => {
                                seek(-300);
                            }
                            "pause\n" => loop {
                                let received = rx.try_recv();
                                match received {
                                    Ok(command) => match command.as_str() {
                                        "play\n" => {
                                            break;
                                        }
                                        &_ => {
                                            println!("Command {:?} undefiend", command)
                                        }
                                    },
                                    Err(_) => {
                                        sleep(Duration::from_millis(10));
                                    }
                                }
                            },
                            &_ => {
                                println!("Command {:?} undefiend", command);
                            }
                        }
                    }
                    Err(_) => {}
                }
                // Write the decoded audio samples to the audio output if the presentation timestamp
                // for the packet is >= the seeked position (0 if not seeking).
                if packet.ts() >= play_opts.seek_ts {
                    if let Some(audio_output) = audio_output {
                        audio_output.write(decoded).unwrap()
                    }
                }
            }
            Err(Error::DecodeError(err)) => {
                // Decode errors are not fatal. Print the error message and try to decode the next
                // packet as usual.
                warn!("decode error: {}", err);
            }
            Err(err) => break,
        }
    }

    // Regardless of result, finalize the decoder to get the verification result.
    let finalize_result = decoder.finalize();
}
