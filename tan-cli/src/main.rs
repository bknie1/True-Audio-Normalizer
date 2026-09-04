mod wav;

use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: tan-cli <input.wav> <output.wav>");
        std::process::exit(1);
    }

    let (spec, samples) = wav::read_wav(&args[1]).expect("failed to read wav");
    println!(
        "{} Hz, {} channel(s), {}-bit, {} samples",
        spec.sample_rate,
        spec.channels,
        spec.bits_per_sample,
        samples.len()
    );

    // Pass-through for now, just to prove read -> write round-trips correctly.
    // The actual normalization goes here once tan-core exists.
    wav::write_wav(&args[2], &spec, &samples).expect("failed to write wav");
}
