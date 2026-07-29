//! Checks `reduce` and `expand` against answers recorded from OpenCV.
//!
//! The crate's conventions were chosen to match `pyrDown`/`pyrUp`, so a
//! disagreement here means imgpyr is wrong, not that the two merely differ.
//! Regenerate with `tests/fixtures/generate.py`.

use imgpyr::{Border, Plane, expand, reduce};

const FIXTURES: &str = include_str!("fixtures/opencv.txt");

/// OpenCV and imgpyr sum in the same order, so agreement should be near exact;
/// this leaves room only for the last bit or two of an `f32`.
const TOLERANCE: f32 = 1e-6;

struct Case {
    operation: String,
    border: Border,
    source: (usize, usize),
    destination: (usize, usize),
    input: Vec<f32>,
    expected: Vec<f32>,
}

impl Case {
    fn describe(&self) -> String {
        let (sw, sh) = self.source;
        let (dw, dh) = self.destination;
        format!(
            "{} {:?} {sw}x{sh} -> {dw}x{dh}",
            self.operation, self.border
        )
    }
}

fn parse() -> Vec<Case> {
    let mut lines = FIXTURES
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty());
    let mut cases = Vec::new();

    while let Some(header) = lines.next() {
        let fields: Vec<&str> = header.split_whitespace().collect();
        assert_eq!(fields[0], "CASE", "unexpected line: {header}");

        let number = |field: &str| field.parse::<usize>().expect("dimension");
        let samples = |line: &str, tag: &str| {
            let (found, values) = line.split_once(' ').expect("tagged sample line");
            assert_eq!(found, tag);
            values
                .split_whitespace()
                .map(|value| value.parse::<f32>().expect("sample"))
                .collect::<Vec<f32>>()
        };

        cases.push(Case {
            operation: fields[1].to_string(),
            border: match fields[2] {
                "replicate" => Border::Replicate,
                "mirror" => Border::Mirror,
                other => panic!("unknown border {other}"),
            },
            source: (number(fields[3]), number(fields[4])),
            destination: (number(fields[5]), number(fields[6])),
            input: samples(lines.next().expect("IN line"), "IN"),
            expected: samples(lines.next().expect("OUT line"), "OUT"),
        });
    }

    cases
}

#[test]
fn reduce_and_expand_agree_with_opencv() {
    let cases = parse();
    assert!(cases.len() >= 20, "fixtures look truncated");

    for case in &cases {
        let source = Plane::from_vec(case.input.clone(), case.source.0, case.source.1);

        let actual = match case.operation.as_str() {
            "reduce" => reduce(&source, case.border),
            "expand" => expand(&source, case.destination.0, case.destination.1, case.border),
            other => panic!("unknown operation {other}"),
        };

        assert_eq!(
            (actual.width(), actual.height()),
            case.destination,
            "{}: wrong output size",
            case.describe()
        );

        let disagreement = actual
            .as_slice()
            .iter()
            .zip(&case.expected)
            .position(|(got, want)| (got - want).abs() > TOLERANCE);

        if let Some(index) = disagreement {
            let (x, y) = (index % actual.width(), index / actual.width());
            panic!(
                "{} disagrees at ({x}, {y}): imgpyr {}, OpenCV {}",
                case.describe(),
                actual.as_slice()[index],
                case.expected[index]
            );
        }
    }
}
