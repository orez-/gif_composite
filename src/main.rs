use atty::Stream;
use gif::{Frame, Encoder, Repeat, SetParameter};
use std::env;
use std::fs::File;
use std::io::stdout;
use std::io::Write;
use std::mem;


fn get_all_same<I, T>(mut iterator: I) -> Option<T>
where
    I: Iterator<Item = T>,
    T: PartialEq,
{
    let first = iterator.next()?;
    for item in iterator {
        if item != first {
            return None;
        }
    }
    Some(first)
}


fn tail<I>(mut iterator: I) -> I
where
    I: Iterator,
{
    iterator.next();
    iterator
}


fn main() {
    if env::args().len() == 1 {
        println!(
            "Pass at least two gifs to composite together. Gifs are layered in the order \
            they're passed: first on bottom, last on top."
        );
        println!(
            "All image files must be the same width and height, and be aligned in \
            frame duration and count."
        );
        return;
    }
    if atty::is(Stream::Stdout) {
        println!(
            "This command returns an image file. Your terminal likely cannot display it directly!"
        );
        println!(
            "Redirect output to a file or `imgcat`. If you really want to see the bytes \
            in your terminal pipe the result to `cat`."
        );
        std::process::exit(1);
    }
    let mut readers = Vec::new();
    for arg in tail(env::args()) {
        let mut decoder = match File::open(&arg) {
            Ok(file) => gif::Decoder::new(file),
            Err(msg) => {
                eprintln!("{} - {}", arg, msg);
                std::process::exit(1)
            }
        };
        // Configure the decoder such that it will expand the image to RGBA.
        decoder.set(gif::ColorOutput::RGBA);
        // Read the file header
        let reader = decoder.read_info().unwrap();
        readers.push(reader);
    }

    let width = get_all_same(readers.iter().map(|reader| reader.width())).unwrap();
    let height = get_all_same(readers.iter().map(|reader| reader.height())).unwrap();

    let mut image = Vec::new();
    let mut encoder = Encoder::new(&mut image, width, height, &[]).unwrap();

    encoder.set(Repeat::Infinite).unwrap();
    loop {
        let maybe_frames: Vec<_> = readers
            .iter_mut()
            .map(|reader| reader.read_next_frame().unwrap())
            .collect();
        if maybe_frames.iter().all(|frame| frame.is_none()) {
            break;
        }
        if !maybe_frames.iter().all(|frame| frame.is_some()) {
            eprintln!("frame count mismatch");
            std::process::exit(1);
        }
        let frames: Vec<_> = maybe_frames
            .into_iter()
            .map(|frame| frame.unwrap())
            .collect();
        let delay = get_all_same(frames.iter().map(|frame| frame.delay)).unwrap();

        let mut buffer = vec![0; width as usize * height as usize * 4];
        // paste each frame overtop the buffer.
        for frame in frames {
            let pixels = buffer.chunks_exact(4).zip(frame.buffer.chunks_exact(4));
            buffer = pixels
                .flat_map(|(left, right)| {
                    match right[3] {
                        0xFF => right,
                        0x00 => left,
                        bad => panic!("Can't handle alpha value of {:?}", bad),
                    }.to_vec()
                })
                .collect();
        }

        let mut composite_frame = Frame::from_rgba(width, height, &mut buffer);
        composite_frame.delay = delay;
        encoder.write_frame(&composite_frame).unwrap();
    }
    mem::drop(encoder);
    stdout().write_all(&image).unwrap();
}
