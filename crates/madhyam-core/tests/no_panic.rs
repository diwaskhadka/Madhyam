//! Smoke "fuzzing": parsers must never panic on hostile or truncated input.
//! Playlists and guides come from the internet; a panic in a TV app is a crash.

use madhyam_core::catalog::{SearchFilter, SourceKind};
use madhyam_core::{Catalog, json, m3u, xmltv};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

const SEEDS: [&str; 3] = [
    "#EXTM3U url-tvg=\"x\"\n#EXTINF:-1 tvg-id=\"a\" group-title=\"G, H\",Name\n#EXTVLCOPT:http-user-agent=UA\n#KODIPROP:k=v\nhttp://h/1.ts|User-Agent=x\n",
    "<?xml version=\"1.0\"?><!DOCTYPE tv><tv><channel id=\"a\"><display-name>A &amp; B</display-name></channel>\
     <programme start=\"20261012100000 +0200\" stop=\"20261012110000 +0200\" channel=\"a\"><title>T</title>\
     <episode-num system=\"xmltv_ns\">1.2.</episode-num><![CDATA[x]]></programme></tv>",
    r#"{"user_info":{"auth":"1"},"a":[1,-2.5e3,"é😀",null,true,{"b":[]}]}"#,
];

fn mutate(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
    let mut v = seed.to_vec();
    for _ in 0..(rng.next() % 8 + 1) {
        if v.is_empty() {
            break;
        }
        let i = (rng.next() % v.len() as u64) as usize;
        match rng.next() % 5 {
            0 => v[i] = rng.next() as u8, // flip a byte
            1 => v.truncate(i),           // truncate
            2 => v.insert(i, b"<>\"'&;=,#:|\n\r\0\xff"[(rng.next() % 15) as usize]), // insert syntax
            3 => {
                let chunk = v[i..].to_vec();
                v.extend_from_slice(&chunk); // duplicate tail
            }
            _ => {
                v.remove(i);
            }
        }
    }
    v
}

#[test]
fn parsers_never_panic() {
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    for round in 0..20_000 {
        let seed = SEEDS[round % SEEDS.len()].as_bytes();
        let input = if round % 10 == 0 {
            (0..rng.next() % 300).map(|_| rng.next() as u8).collect()
        } else {
            mutate(&mut rng, seed)
        };

        let _ = m3u::M3uReader::new(&input[..]).count();
        let _ = xmltv::XmltvReader::new(&input[..]).take(10_000).count();
        let _ = json::parse_bytes(&input);

        let mut c = Catalog::new();
        let s = c.add_source(
            "f",
            SourceKind::M3u {
                location: String::new(),
            },
        );
        let _ = c.import_m3u(s, &input[..]);
        let _ = c.import_xmltv(&input[..], 0);
        let text = String::from_utf8_lossy(&input);
        let _ = c.search(&text, SearchFilter::default(), 5);
        let _ = Catalog::load_snapshot(&input[..]);
    }
}
