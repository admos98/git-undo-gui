//! Fuzz-style tests for the two porcelain parsers.
//!
//! Both data-loss bugs this repository has shipped lived in these two functions, and
//! neither was found by the hand-written tests that existed at the time — 45 passing
//! tests in this crate did not catch either:
//!
//! 1. `parse_porcelain` kept the quotes from git's C-quoted line format, so an
//!    untracked file whose name contained a space (or a quote, or a non-ASCII byte)
//!    did not match the same path as printed by `ls-tree`, the guard concluded the
//!    target snapshot did not track it, and `git reset --hard` overwrote it.
//! 2. `git status --porcelain` without `--ignored` reports *nothing* for an ignored
//!    file, so a committed-then-gitignored `.env` was invisible and a hard reset
//!    replaced it with the committed version.
//!
//! Both are the same shape of failure: a parser that silently agreed with the wrong
//! input. Hand-written examples test the cases the author thought of; the generators
//! below test the ones they did not. The invariants asserted here are deliberately
//! narrow and absolute, because "usually right about paths" is what cost the data:
//!
//! * `parse_porcelain_z` round-trips **every** path byte-for-byte, including spaces,
//!   quotes, tabs, newlines and non-ASCII — for any path it can build.
//! * Neither parser panics on truncated, empty or garbage input.
//! * Rename and copy entries never produce a phantom second entry from their origin
//!   field, which would make the guard see a file that does not exist and, worse,
//!   miss the one that does.
//!
//! Deliberately dependency-free. `rand` would be the obvious choice and is refused:
//! the whole point of this crate is that `cargo test --no-default-features` builds it
//! with zero external crates, so the safety rules can be audited offline.

use git_undo_logic::state::{parse_porcelain, parse_porcelain_z};

/// Deterministic xorshift64*. Seeded per test, so a failure reproduces exactly.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Rng {
        // A zero state is a fixed point for xorshift; any odd seed is fine.
        Rng(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len())]
    }
}

/// Path fragments chosen because each one has broken a real tool at least once:
/// spaces (the shipped bug), quotes, tabs, newlines, non-ASCII, a leading dash that
/// could be read as a flag, `$(` and `;` for anyone who interpolates into a shell,
/// and the `--help` argument itself.
const PIECES: &[&str] = &[
    "notes",
    "my notes",
    "a",
    "src",
    "lib",
    "café",
    "日本",
    "quo\"te",
    "apo'strophe",
    "back\\slash",
    "-dash",
    "--help",
    "tab\there",
    " lead",
    "trail ",
    "dot.",
    "a b/c d",
    "line\nbreak",
    "unicode✓",
    "space  two",
    "semi;colon",
    "$(whoami)",
    "`tick`",
    "…",
    "\\0notreally",
    "*star*",
    "?question",
];

fn random_path(rng: &mut Rng) -> String {
    let pieces = 1 + rng.below(3);
    let mut out = String::new();
    for i in 0..pieces {
        if i > 0 {
            out.push_str(rng.pick(&["/", ".", " ", "-", ""]));
        }
        out.push_str(rng.pick(PIECES));
    }
    out
}

/// The statuses `-z` can emit for an ordinary entry, paired with what the parser
/// must call them.
const SIMPLE_STATUSES: &[(&str, &str)] = &[
    ("??", "untracked"),
    ("!!", "ignored"),
    (" M", "modified"),
    ("M ", "staged"),
    ("MM", "staged+modified"),
    (" D", "deleted"),
    ("D ", "staged deletion"),
    ("A ", "added"),
];

#[test]
fn the_z_parser_round_trips_every_path_it_can_be_given() {
    // The invariant whose violation destroyed data: the parser must hand back the
    // same bytes git put in. A single altered path is enough for the guard to
    // compare it against the wrong tree entry.
    let mut rng = Rng::new(0x5eed_1234);
    let mut checked = 0usize;

    for _ in 0..20_000 {
        let path = random_path(&mut rng);
        let (wire_status, expected_status) = *rng.pick(SIMPLE_STATUSES);
        // `git status --porcelain=v1 -z`: two status bytes, one space, literal path,
        // NUL. This is the format the guard depends on for every data-loss decision.
        let wire = format!("{wire_status} {path}\0");

        let parsed = parse_porcelain_z(&wire);
        assert_eq!(parsed.len(), 1, "one entry expected for {path:?}, got {parsed:?}");
        assert_eq!(
            parsed[0].path, path,
            "the parser altered the path git supplied: {:?} became {:?}",
            path, parsed[0].path
        );
        assert_eq!(
            parsed[0].status, expected_status,
            "status {wire_status:?} for {path:?} was classified as {:?}",
            parsed[0].status
        );
        checked += 1;
    }

    assert!(checked == 20_000, "the generator stopped early: {checked}");
}

#[test]
fn the_z_parser_does_not_strip_quotes_that_are_part_of_the_name() {
    // The original bug was the parser silently *keeping* git's added quotes. The
    // opposite fix — stripping quotes unconditionally — would corrupt any filename
    // that legitimately contains one, which is just the same bug pointing the other
    // way. `-z` output is never quoted, so a quote can only be a real character.
    let cases = [
        ("quo\"te.md", "quo\"te.md"),
        ("\"quoted.md\"", "\"quoted.md\""),
        ("a\"b\"c", "a\"b\"c"),
        ("plain.md", "plain.md"),
    ];
    for (on_disk, expected) in cases {
        let parsed = parse_porcelain_z(&format!("?? {on_disk}\0"));
        assert_eq!(parsed.len(), 1, "{on_disk:?}");
        assert_eq!(parsed[0].path, expected, "{on_disk:?} must survive verbatim");
    }
}

#[test]
fn rename_and_copy_origins_never_become_phantom_entries() {
    // `R  new\0old\0` — the origin is its own field. If it were read as a separate
    // entry the guard would see a path that does not exist, and the count it bases
    // its "N files at risk" message on would be wrong.
    let mut rng = Rng::new(0xbeef_0001);

    for _ in 0..2_000 {
        let new = random_path(&mut rng);
        // A rename from a path to itself is not something git emits, and generating it
        // would make the "origin must not appear" assertion fail on a coincidence
        // rather than on a parser bug.
        let mut old = random_path(&mut rng);
        while old == new {
            old = random_path(&mut rng);
        }
        // `after` must differ from both, or the "origin must not appear" assertion
        // fires on a coincidence instead of on a parser bug.
        let mut after = random_path(&mut rng);
        while after == new || after == old {
            after = random_path(&mut rng);
        }
        let marker = *rng.pick(&["R ", "C "]);
        // new, origin, then a genuinely separate entry to prove the loop resynchronises.
        let wire = format!("{marker} {new}\0{old}\0?? {after}\0");

        let parsed = parse_porcelain_z(&wire);
        assert_eq!(parsed.len(), 2, "expected the rename and the entry after it: {parsed:?}");
        assert_eq!(parsed[0].path, new, "{parsed:?}");
        assert_eq!(parsed[1].path, after, "the loop lost sync after a rename: {parsed:?}");
        assert!(
            parsed.iter().all(|f| f.path != old),
            "the origin field was emitted as an entry (new={new:?} old={old:?}): {parsed:?}"
        );
    }
}

#[test]
fn mixed_entry_streams_keep_their_count_and_order() {
    // A single wrong split anywhere shifts every later entry, so a mixed stream is the
    // cheapest way to catch an off-by-one that a uniform stream would hide. The origin
    // field only exists for `R`/`C`, so it is emitted only there — generating one
    // anywhere else would be testing a stream git cannot produce.
    let mut rng = Rng::new(0x1234_5678);
    let rename_statuses: &[(&str, &str)] = &[("R ", "renamed"), ("C ", "copied")];

    for _ in 0..1_000 {
        let n = 1 + rng.below(12);
        let mut expected: Vec<String> = Vec::with_capacity(n);
        let mut wire = String::new();
        for _ in 0..n {
            let path = random_path(&mut rng);
            expected.push(path.clone());

            if rng.below(3) == 0 {
                let (status, _) = *rng.pick(rename_statuses);
                let mut origin = random_path(&mut rng);
                while origin == path {
                    origin = random_path(&mut rng);
                }
                // <status><space><new>\0<origin>\0 — the extra field is the one most
                // likely to be mistaken for the next entry.
                wire.push_str(&format!("{status} {path}\0{origin}\0"));
            } else {
                let (status, _) = *rng.pick(SIMPLE_STATUSES);
                wire.push_str(&format!("{status} {path}\0"));
            }
        }

        let parsed = parse_porcelain_z(&wire);
        let got: Vec<String> = parsed.iter().map(|f| f.path.clone()).collect();
        assert_eq!(got, expected, "entry stream did not survive parsing");
    }
}

#[test]
fn neither_parser_panics_on_truncated_or_garbage_input() {
    // Refusals are cheap; a panic in the guard would take down the command that was
    // supposed to be the safe one. Fuzz the shape of the input, not its meaning.
    let mut rng = Rng::new(0xdead_beef);
    let mut buffer = String::new();

    for _ in 0..20_000 {
        buffer.clear();
        let len = rng.below(40);
        for _ in 0..len {
            let byte = match rng.below(8) {
                0 => '\0',
                1 => ' ',
                2 => '\n',
                3 => '\r',
                4 => '\t',
                5 => *rng.pick(&['R', 'C', '?', '!', 'M', 'D', 'A', 'U', ' ']),
                6 => *rng.pick(&['a', '/', '.', '"', '\\', 'é', '#']),
                _ => (rng.below(95) as u8 + 32) as char,
            };
            buffer.push(byte);
        }

        // Both must return without panicking on anything at all.
        let _ = parse_porcelain_z(&buffer);
        let _ = parse_porcelain(&buffer);
    }
}

#[test]
fn the_line_parser_is_only_trusted_for_paths_git_would_not_quote() {
    // This is the contract that makes the `-z` parser worth its extra code. The line
    // format C-quotes a path that contains a space, a quote or a non-ASCII byte, and
    // this parser does not unquote, so it reports a path that does not exist on disk.
    //
    // The assertion is deliberately two-sided: it pins the *difference* between the
    // parsers. If someone later "simplifies" the line parser to strip quotes, the
    // second half fails; if someone swaps it back in for repository state, the first
    // half documents exactly which files that loses.
    let mut rng = Rng::new(0x0ff1ce);
    let simple = ["notes.md", "src/a.ts", "keep", "a.b.c", "UPPER"];

    for _ in 0..2_000 {
        let path = *rng.pick(&simple);
        let (status, expected) = *rng.pick(SIMPLE_STATUSES);
        let line = format!("{status} {path}\n");

        let z = parse_porcelain_z(&format!("{status} {path}\0"));
        assert_eq!(z.len(), 1);
        assert_eq!(z[0].path, path, "the -z parser must be exact even for simple paths");

        let parsed = parse_porcelain(&line);
        assert_eq!(parsed.len(), 1, "{line:?}");
        assert_eq!(
            parsed[0].path, expected_path(path),
            "the line parser is expected to be exact only for unquoted paths"
        );
        let _ = expected;
    }

    // And the divergence itself, stated once, concretely.
    let quoted = parse_porcelain("?? \"my notes.md\"\n");
    assert_eq!(quoted[0].path, "\"my notes.md\"", "the quotes are part of what it returns");
    assert_ne!(quoted[0].path, "my notes.md", "this is why state must use -z");

    let unquoted = parse_porcelain_z("?? my notes.md\0");
    assert_eq!(unquoted[0].path, "my notes.md", "and this is why -z is used instead");
}

fn expected_path(path: &str) -> String {
    // The line parser trims the path after skipping the three status bytes.
    path.to_string()
}
