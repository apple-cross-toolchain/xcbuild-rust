use std::env;
use std::process;
use xcbuild_car::{dump_facet, dump_rendition, CarReader};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("usage: dump_car <file.car>");
        process::exit(1);
    }

    let path = &args[0];

    let car = match CarReader::open(path) {
        Some(c) => c,
        None => {
            eprintln!("error: unable to load CAR archive '{path}'");
            process::exit(1);
        }
    };

    // Dump variables
    for (name, size) in car.variables() {
        println!("Variable: {name} [{size:08x}]");
    }
    println!();

    // Dump header
    car.dump_header();
    println!();

    // Dump key format
    car.dump_key_format();
    println!();

    // Dump facets and their renditions
    let mut facet_count = 0;
    let mut rendition_count = 0;

    for facet in &car.facets {
        facet_count += 1;
        dump_facet(facet);

        let renditions = car.lookup_renditions(facet);
        for r in renditions {
            rendition_count += 1;
            dump_rendition(r);
        }
    }

    println!("\nFound {facet_count} facets and {rendition_count} renditions");
}
