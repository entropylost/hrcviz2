//! Renders the example scene to a PNG without a browser.
//!
//! Usage: offline [-o OUTPUT] [-t TRANSMISSION] [-f FRUSTRUM_START] [-s SCALE]

use std::process::exit;

use image::{GrayImage, Luma};
use twilight::render::{DEFAULT_FRUSTRUM_START, TransmissionKind};
use twilight::scene::Scene;

const SIZE: u32 = 128;

fn usage() -> ! {
    let names = TransmissionKind::ALL
        .iter()
        .map(|kind| kind.name())
        .collect::<Vec<_>>()
        .join(", ");
    eprintln!("Usage: offline [-o OUTPUT] [-t TRANSMISSION] [-f FRUSTRUM_START] [-s SCALE]");
    eprintln!("  -o  output path (default: render.png)");
    eprintln!("  -t  one of {names} (default: {})", TransmissionKind::default().name());
    eprintln!("  -f  frustrum length (default: {DEFAULT_FRUSTRUM_START})");
    eprintln!("  -s  pixels per cell (default: 1)");
    exit(1);
}

fn main() {
    let mut output = "render.png".to_string();
    let mut transmission = TransmissionKind::default();
    let mut frustrum_start = DEFAULT_FRUSTRUM_START;
    let mut scale = 1;

    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let Some(value) = args.next() else { usage() };
        match flag.as_str() {
            "-o" => output = value,
            "-t" => {
                transmission = value.parse().unwrap_or_else(|err| {
                    eprintln!("{err}");
                    usage()
                })
            }
            "-f" => frustrum_start = value.parse().unwrap_or_else(|_| usage()),
            "-s" => scale = value.parse().unwrap_or_else(|_| usage()),
            _ => usage(),
        }
    }
    let radiance = transmission.render(&Scene::example(), frustrum_start);

    let image = GrayImage::from_fn(SIZE * scale, SIZE * scale, |x, y| {
        let cell = glam::UVec2::new(x / scale, y / scale).as_ivec2();
        let value = radiance.get(&cell).copied().unwrap_or(0.0);
        Luma([(value.clamp(0.0, 1.0) * 255.0).round() as u8])
    });
    image.save(&output).unwrap_or_else(|err| {
        eprintln!("Failed to write {output}: {err}");
        exit(1);
    });
    println!("Wrote {output}");
}
