//! `iptv` — inspect playlists and guides, and benchmark the core.
//!
//! ```text
//! madhyam_core parse-m3u  <playlist.m3u>
//! madhyam_core parse-epg  <guide.xml>
//! madhyam_core search     <playlist.m3u> <query...>
//! madhyam_core now        <playlist.m3u> <guide.xml> [query...]
//! madhyam_core bench      [channels=100000] [programmes_per_channel=50]
//! ```

use madhyam_core::catalog::{SearchFilter, SourceKind};
use madhyam_core::m3u::M3uReader;
use madhyam_core::xmltv::{XmltvItem, XmltvReader};
use madhyam_core::{Catalog, ItemKind, Result, time};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::BufReader;
use std::time::Instant;

fn open(path: &str) -> Result<BufReader<std::fs::File>> {
    Ok(BufReader::with_capacity(
        256 * 1024,
        std::fs::File::open(path)?,
    ))
}

fn ms(t: Instant) -> String {
    format!("{:.1} ms", t.elapsed().as_secs_f64() * 1000.0)
}

fn load(m3u: &str) -> Result<(Catalog, u32)> {
    let mut c = Catalog::new();
    let s = c.add_source(
        m3u,
        SourceKind::M3u {
            location: m3u.into(),
        },
    );
    c.import_m3u(s, open(m3u)?)?;
    Ok((c, s))
}

fn parse_m3u(path: &str) -> Result<()> {
    let t = Instant::now();
    let mut r = M3uReader::new(open(path)?);
    let (mut ok, mut bad) = (0usize, 0usize);
    let mut kinds: HashMap<&'static str, usize> = HashMap::new();
    let mut groups: HashMap<String, usize> = HashMap::new();
    for e in r.by_ref() {
        match e {
            Ok(e) => {
                ok += 1;
                *kinds.entry(e.guess_kind().as_str()).or_default() += 1;
                *groups.entry(e.group).or_default() += 1;
            }
            Err(_) => bad += 1,
        }
    }
    println!("entries: {ok} ({bad} malformed) in {}", ms(t));
    println!("kinds:   {kinds:?}");
    println!("groups:  {}", groups.len());
    let mut top: Vec<_> = groups.into_iter().collect();
    top.sort_by_key(|g| std::cmp::Reverse(g.1));
    for (g, n) in top.iter().take(10) {
        println!("  {n:>7}  {g}");
    }
    let epg = r.header().epg_urls();
    if !epg.is_empty() {
        println!("epg urls: {}", epg.join(", "));
    }
    Ok(())
}

fn parse_epg(path: &str) -> Result<()> {
    let t = Instant::now();
    let mut r = XmltvReader::new(open(path)?);
    let (mut ch, mut pr) = (0usize, 0usize);
    let (mut first, mut last) = (i64::MAX, i64::MIN);
    for it in r.by_ref() {
        match it? {
            XmltvItem::Channel(_) => ch += 1,
            XmltvItem::Programme(p) => {
                pr += 1;
                first = first.min(p.start);
                last = last.max(p.stop.max(p.start));
            }
        }
    }
    println!(
        "channels: {ch}, programmes: {pr} ({} skipped) in {}",
        r.skipped,
        ms(t)
    );
    if pr > 0 {
        let f = time::parts_from_unix(first);
        let l = time::parts_from_unix(last);
        println!(
            "range: {}-{:02}-{:02} .. {}-{:02}-{:02} (UTC)",
            f.0, f.1, f.2, l.0, l.1, l.2
        );
    }
    Ok(())
}

fn search(m3u: &str, q: &str) -> Result<()> {
    let (c, _) = load(m3u)?;
    let t = Instant::now();
    let hits = c.search(q, SearchFilter::default(), 25);
    println!("{} hits in {}", hits.len(), ms(t));
    for h in hits {
        println!("  [{}] {}  ({})", h.kind.as_str(), h.name(), h.group);
    }
    Ok(())
}

fn now_playing(m3u: &str, xml: &str, q: &str) -> Result<()> {
    let (mut c, s) = load(m3u)?;
    let now = time::now();
    let st = c.import_xmltv(open(xml)?, now - 3600)?;
    println!(
        "guide: {} channels, {} programmes kept",
        st.channels, st.programmes
    );
    let items: Vec<_> = if q.is_empty() {
        c.list(s, ItemKind::Live, None, 0, 30)
    } else {
        c.search(
            q,
            SearchFilter {
                kind: Some(ItemKind::Live),
                source: None,
            },
            30,
        )
    };
    for it in items {
        let (n, x) = c.now_next(it.id, now);
        println!(
            "{:<32} now: {:<30} next: {}",
            it.name(),
            n.as_ref().map_or("-", |p| p.title.as_str()),
            x.as_ref().map_or("-", |p| p.title.as_str())
        );
    }
    Ok(())
}

// ---------------------------------------------------------------- bench

const WORDS: [&str; 24] = [
    "news", "sport", "cinema", "kids", "music", "docu", "world", "action", "drama", "comedy",
    "live", "premium", "télé", "münchen", "arena", "nature", "history", "travel", "food",
    "science", "max", "plus", "one", "gold",
];
const COUNTRIES: [&str; 12] = [
    "UK", "US", "FR", "DE", "ES", "IT", "TR", "AR", "IN", "BR", "PL", "NL",
];

/// Tiny deterministic PRNG (xorshift64*) so benchmarks are reproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn pick<'a>(&mut self, v: &[&'a str]) -> &'a str {
        v[(self.next() % v.len() as u64) as usize]
    }
}

fn bench(channels: usize, per: usize) -> Result<()> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    println!(
        "generating {channels} entries and {} programmes...",
        channels / 4 * per
    );

    let mut m3u = String::with_capacity(channels * 200);
    m3u.push_str("#EXTM3U url-tvg=\"http://epg.example/guide.xml\"\n");
    let mut country = Vec::with_capacity(channels);
    for i in 0..channels {
        let c = rng.pick(&COUNTRIES);
        country.push(c);
        let name = format!("{c}: {} {} {i}", rng.pick(&WORDS), rng.pick(&WORDS));
        let movie = i % 4 == 3;
        let _ = writeln!(
            m3u,
            "#EXTINF:-1 tvg-id=\"ch{i}.{c}\" tvg-name=\"{name}\" tvg-logo=\"http://logo.example/{i}.png\" group-title=\"{c} | {}\",{name}\nhttp://stream.example:8080/{}/user/pass/{i}.{}",
            rng.pick(&WORDS),
            if movie { "movie" } else { "live" },
            if movie { "mkv" } else { "ts" },
        );
    }

    let base = time::now() / 3600 * 3600 - 6 * 3600;
    let mut xml = String::with_capacity(channels / 4 * per * 260);
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<tv>\n");
    // Guide for a quarter of the channels (typical: providers cover only part).
    let guided: Vec<usize> = (0..channels).filter(|i| i % 4 == 0).collect();
    for &i in &guided {
        let c = country[i];
        let _ = writeln!(
            xml,
            "<channel id=\"ch{i}.{c}\"><display-name>Ch {i}</display-name></channel>"
        );
    }
    for &i in &guided {
        let c = country[i];
        let mut t = base;
        for k in 0..per {
            let len = 1800 * (1 + (rng.next() % 4) as i64);
            let (s, e) = (time::parts_from_unix(t), time::parts_from_unix(t + len));
            let _ = writeln!(
                xml,
                "<programme start=\"{:04}{:02}{:02}{:02}{:02}00 +0000\" stop=\"{:04}{:02}{:02}{:02}{:02}00 +0000\" channel=\"ch{i}.{c}\"><title>{} &amp; {} #{k}</title><desc>Episode {k} of a {} show.</desc><category>{}</category></programme>",
                s.0,
                s.1,
                s.2,
                s.3,
                s.4,
                e.0,
                e.1,
                e.2,
                e.3,
                e.4,
                rng.pick(&WORDS),
                rng.pick(&WORDS),
                rng.pick(&WORDS),
                rng.pick(&WORDS)
            );
            t += len;
        }
    }
    xml.push_str("</tv>\n");
    println!(
        "playlist {:.1} MB, guide {:.1} MB\n",
        m3u.len() as f64 / 1e6,
        xml.len() as f64 / 1e6
    );

    let mut c = Catalog::new();
    let src = c.add_source(
        "bench",
        SourceKind::M3u {
            location: String::new(),
        },
    );

    let t = Instant::now();
    let st = c.import_m3u(src, m3u.as_bytes())?;
    println!(
        "{:<34}{:>12}   ({} items)",
        "M3U parse + index",
        ms(t),
        st.items
    );

    let t = Instant::now();
    let e = c.import_xmltv(xml.as_bytes(), base)?;
    println!(
        "{:<34}{:>12}   ({} programmes)",
        "XMLTV parse + normalise",
        ms(t),
        e.programmes
    );

    let queries = [
        "new",
        "sport arena",
        "tele",
        "munchen gold",
        "kids 12",
        "uk: cin",
        "zzzz",
        "a",
    ];
    let t = Instant::now();
    let rounds = 50;
    let mut hits = 0;
    for _ in 0..rounds {
        for q in queries {
            hits += c.search(q, SearchFilter::default(), 50).len();
        }
    }
    let per_q = t.elapsed().as_secs_f64() * 1000.0 / (rounds * queries.len()) as f64;
    println!(
        "{:<34}{:>9.3} ms   (avg over {} queries, {hits} hits)",
        "search (top 50)",
        per_q,
        rounds * queries.len()
    );

    let live = c.list(src, ItemKind::Live, None, 0, usize::MAX);
    let now = time::now();
    let t = Instant::now();
    let found = live
        .iter()
        .filter(|it| c.now_next(it.id, now).0.is_some())
        .count();
    println!(
        "{:<34}{:>12}   ({} channels, {found} on air)",
        "now/next for every live channel",
        ms(t),
        live.len()
    );

    let t = Instant::now();
    let g = c.groups(src, ItemKind::Live);
    println!(
        "{:<34}{:>12}   ({} groups)",
        "group listing",
        ms(t),
        g.len()
    );

    let t = Instant::now();
    let mut snap = Vec::new();
    c.save_snapshot(&mut snap)?;
    let save = ms(t);
    let t = Instant::now();
    let d = Catalog::load_snapshot(&snap[..])?;
    println!(
        "{:<34}{:>12}   ({:.1} MB)",
        "snapshot save",
        save,
        snap.len() as f64 / 1e6
    );
    println!(
        "{:<34}{:>12}   ({} items)",
        "snapshot load + reindex",
        ms(t),
        d.len()
    );
    Ok(())
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  iptv parse-m3u <file>\n  iptv parse-epg <file>\n  iptv search <m3u> <query>\n  \
         iptv now <m3u> <xmltv> [query]\n  iptv bench [channels] [programmes_per_channel]"
    );
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = |i: usize| args.get(i).map(String::as_str);
    let rest = |i: usize| args.get(i..).map(|r| r.join(" ")).unwrap_or_default();
    let r = match (a(0), a(1), a(2)) {
        (Some("parse-m3u"), Some(f), _) => parse_m3u(f),
        (Some("parse-epg"), Some(f), _) => parse_epg(f),
        (Some("search"), Some(f), Some(_)) => search(f, &rest(2)),
        (Some("now"), Some(m), Some(x)) => now_playing(m, x, &rest(3)),
        (Some("bench"), _, _) => {
            let n = a(1).and_then(|s| s.parse().ok()).unwrap_or(100_000);
            let p = a(2).and_then(|s| s.parse().ok()).unwrap_or(50);
            bench(n, p)
        }
        _ => usage(),
    };
    if let Err(e) = r {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
