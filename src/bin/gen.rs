use std::time::Instant;

use hrcviz2::generate_merging_scheme;

fn main() {
    let start = Instant::now();
    let merging_scheme = generate_merging_scheme(6);
    println!("Elapsed: {:.2?}", start.elapsed());

    std::fs::write(
        "merging_scheme.bin",
        postcard::to_allocvec(&merging_scheme).unwrap(),
    )
    .unwrap();

    let serde_merging_scheme = merging_scheme
        .iter()
        .map(|(k, v)| (*k, v.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>()))
        .collect::<Vec<_>>();

    std::fs::write(
        "merging_scheme.json",
        serde_json::to_string(&serde_merging_scheme).unwrap(),
    )
    .unwrap();
}
