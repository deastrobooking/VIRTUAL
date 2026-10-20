//! The graphing calculator's Math Library: ready-made examples, organised in
//! folders by field, each with a sentence or two explaining the idea. Pure
//! data, so a test can check that every example parses and draws.

use std::f32::consts::{PI, TAU};

/// One example: the input lines to add, plus the ranges that show it best.
#[derive(Clone, Copy, Debug)]
pub struct LibraryEntry {
    pub title: &'static str,
    pub blurb: &'static str,
    pub lines: &'static [&'static str],
    /// Slider ranges by name: `(name, min, max)`.
    pub sliders: &'static [(&'static str, f32, f32)],
    /// The t, θ, u or n range for objects with a parameter.
    pub parameter: Option<[f32; 2]>,
    /// The v range for parametric surfaces.
    pub secondary: Option<[f32; 2]>,
    /// The visible region `[x0, x1, y0, y1]`; the x/y domain in 3D.
    pub window: Option<[f32; 4]>,
    /// Whether the example moves with `time`.
    pub animate: bool,
}

const fn entry(
    title: &'static str,
    blurb: &'static str,
    lines: &'static [&'static str],
) -> LibraryEntry {
    LibraryEntry {
        title,
        blurb,
        lines,
        sliders: &[],
        parameter: None,
        secondary: None,
        window: None,
        animate: false,
    }
}

impl LibraryEntry {
    const fn sliders(self, sliders: &'static [(&'static str, f32, f32)]) -> Self {
        Self { sliders, ..self }
    }

    const fn t(self, start: f32, end: f32) -> Self {
        Self {
            parameter: Some([start, end]),
            ..self
        }
    }

    const fn v(self, start: f32, end: f32) -> Self {
        Self {
            secondary: Some([start, end]),
            ..self
        }
    }

    const fn window(self, x0: f32, x1: f32, y0: f32, y1: f32) -> Self {
        Self {
            window: Some([x0, x1, y0, y1]),
            ..self
        }
    }

    /// A square 3D domain `[-half, half]²`.
    const fn cube(self, half: f32) -> Self {
        self.window(-half, half, -half, half)
    }

    const fn animate(self) -> Self {
        Self {
            animate: true,
            ..self
        }
    }

    /// The slider range for `name`, if the example sets one.
    pub fn slider_range(&self, name: &str) -> Option<[f32; 2]> {
        self.sliders
            .iter()
            .find(|(slider, ..)| *slider == name)
            .map(|&(_, min, max)| [min, max])
    }
}

/// A folder of examples and subfolders.
#[derive(Clone, Copy, Debug)]
pub struct LibraryFolder {
    pub name: &'static str,
    pub blurb: &'static str,
    pub folders: &'static [LibraryFolder],
    pub entries: &'static [LibraryEntry],
}

const fn folder(
    name: &'static str,
    blurb: &'static str,
    entries: &'static [LibraryEntry],
) -> LibraryFolder {
    LibraryFolder {
        name,
        blurb,
        folders: &[],
        entries,
    }
}

const fn parent(
    name: &'static str,
    blurb: &'static str,
    folders: &'static [LibraryFolder],
) -> LibraryFolder {
    LibraryFolder {
        name,
        blurb,
        folders,
        entries: &[],
    }
}

impl LibraryFolder {
    /// Examples in this folder and every subfolder.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
            + self
                .folders
                .iter()
                .map(LibraryFolder::entry_count)
                .sum::<usize>()
    }

    /// Visits every example with its folder path.
    pub fn walk<'a>(
        &'a self,
        path: &mut Vec<&'a str>,
        visit: &mut impl FnMut(&[&str], &'a LibraryEntry),
    ) {
        path.push(self.name);
        for entry in self.entries {
            visit(path, entry);
        }
        for folder in self.folders {
            folder.walk(path, visit);
        }
        path.pop();
    }
}

/// Every example whose title, explanation, folder or equations contain all
/// the words of `query` (case-insensitive), with its folder path.
pub fn search(query: &str) -> Vec<(String, &'static LibraryEntry)> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    let mut found = Vec::new();
    for root in LIBRARY {
        root.walk(&mut Vec::new(), &mut |path, entry| {
            let path = path.join(" › ");
            let haystack = format!(
                "{} {} {} {}",
                entry.title,
                entry.blurb,
                path,
                entry.lines.join(" ")
            )
            .to_lowercase();
            if words.iter().all(|word| haystack.contains(word.as_str())) {
                found.push((path, entry));
            }
        });
    }
    found
}

/// Examples across the whole library.
pub fn entry_count() -> usize {
    LIBRARY.iter().map(LibraryFolder::entry_count).sum()
}

pub static LIBRARY: &[LibraryFolder] = &[
    folder(
        "Number Theory",
        "Whole numbers, primes and the patterns hiding inside them.",
        &[
            entry(
                "Prime barcode",
                "A dot at height 1 marks each prime. Primes thin out as numbers grow, but never stop: Euclid proved there are infinitely many.",
                &["(n, isprime(n))"],
            )
            .t(1.0, 60.0)
            .window(0.0, 61.0, -0.5, 1.5),
            entry(
                "Prime counting π(x)",
                "The staircase counts primes up to x. The prime number theorem says it grows like x / ln(x), the smooth curve.",
                &["y = sum(k, 2, floor(x), isprime(k))", "y = x/ln(x)"],
            )
            .window(0.0, 100.0, -2.0, 30.0),
            entry(
                "Sacks prime spiral",
                "Number n sits at radius √n, turning once per square number. Primes line up along curves, a pattern nobody fully understands.",
                &["(sqrt(n) cos(2pi sqrt(n))/isprime(n), sqrt(n) sin(2pi sqrt(n))/isprime(n))"],
            )
            .t(1.0, 2500.0)
            .window(-55.0, 55.0, -35.0, 35.0),
            entry(
                "Divisor count",
                "How many whole numbers divide n. Primes score exactly 2; highly composite numbers like 12, 24, 36, 48 and 60 spike.",
                &["(n, sum(k, 1, n, 1 - sign(mod(n, k))))"],
            )
            .t(1.0, 60.0)
            .window(0.0, 61.0, -1.0, 13.0),
            entry(
                "Euler's totient φ(n)",
                "Counts the numbers up to n sharing no factor with n. Primes reach the top line φ(p) = p − 1.",
                &["(n, sum(k, 1, n, 1 - sign(gcd(n, k) - 1)))", "y = x - 1"],
            )
            .t(1.0, 60.0)
            .window(0.0, 61.0, -2.0, 62.0),
            entry(
                "Coprime lattice",
                "Shaded squares mark grid points (x, y) with no common factor: the points you can see from the origin. About 61% (6/π²) of them.",
                &["gcd(floor(x), floor(y)) < 1.5"],
            )
            .window(0.0, 30.0, 0.0, 18.0),
            entry(
                "Fibonacci numbers",
                "Each Fibonacci number is the sum of the two before. Binet's formula computes them directly from the golden ratio.",
                &["(n, ((1 + sqrt(5))/2)^n/sqrt(5) - ((1 - sqrt(5))/2)^n/sqrt(5))"],
            )
            .t(0.0, 15.0)
            .window(-1.0, 16.0, -20.0, 620.0),
            entry(
                "Golden ratio from Fibonacci",
                "Ratios of neighbouring Fibonacci numbers zig-zag onto φ = 1.618…, the line.",
                &[
                    "p = (1 + sqrt(5))/2",
                    "q = (1 - sqrt(5))/2",
                    "(n, (p^(n + 1) - q^(n + 1))/(p^n - q^n))",
                    "y = p",
                ],
            )
            .t(1.0, 15.0)
            .window(0.0, 16.0, 0.8, 2.2),
            entry(
                "Golden spiral",
                "A logarithmic spiral that grows by the golden ratio every quarter turn, close to the spiral of a nautilus shell.",
                &["p = (1 + sqrt(5))/2", "r = p^(2θ/pi)"],
            )
            .t(-8.0, 8.0),
            entry(
                "Digit sum",
                "Add the decimal digits of n. The saw-tooth resets every 10, with bigger drops every 100.",
                &["(n, sum(k, 0, 4, floor(n/10^k) - 10 floor(n/10^(k + 1))))"],
            )
            .t(0.0, 120.0)
            .window(-2.0, 122.0, -1.0, 20.0),
            entry(
                "Times tables on a circle",
                "Join point k to point p·k around a 60-point clock. p = 2 draws a cardioid, 3 a nephroid; slide p slowly for a show.",
                &[
                    "p = 2",
                    "x^2 + y^2 = 1",
                    "(cos(2pi floor(t)/60) + (t - floor(t))/0.95 (cos(2pi p floor(t)/60) - cos(2pi floor(t)/60)) + 0 sqrt(1 - (t - floor(t))/0.95), sin(2pi floor(t)/60) + (t - floor(t))/0.95 (sin(2pi p floor(t)/60) - sin(2pi floor(t)/60)))",
                ],
            )
            .sliders(&[("p", 0.0, 10.0)])
            .t(0.0, 60.0)
            .window(-2.4, 2.4, -1.4, 1.4),
            entry(
                "Floor, ceiling and round",
                "Three ways to turn a number into a whole number: always down, always up, or to the nearest.",
                &["y = floor(x)", "y = ceil(x) + 0.1", "y = round(x) - 0.1"],
            )
            .window(-5.0, 5.0, -5.0, 5.0),
            entry(
                "Modular arithmetic",
                "x mod m wraps the number line around a clock of size m, like hours wrapping at 12.",
                &["m = 3", "y = mod(x, m)"],
            )
            .sliders(&[("m", 0.5, 8.0)]),
            entry(
                "Squares and triangular numbers",
                "Square numbers n² and triangular numbers n(n + 1)/2, the counts of dots in squares and triangles.",
                &["(n, n^2)", "(n, n(n + 1)/2)"],
            )
            .t(0.0, 12.0)
            .window(-1.0, 13.0, -5.0, 150.0),
            entry(
                "Odd numbers make squares",
                "1 + 3 + 5 + … + (2n − 1) = n². Each odd number adds an L-shaped layer to a square.",
                &["(n, sum(k, 1, n, 2k - 1))", "y = x^2"],
            )
            .t(1.0, 10.0)
            .window(0.0, 11.0, -5.0, 105.0),
        ],
    ),
    parent(
        "Algebra",
        "Equations, lines, parabolas and the tools for solving them.",
        &[
            folder(
                "Linear",
                "Straight lines and the equations behind them.",
                &[
                    entry(
                        "Slope-intercept form",
                        "y = m x + b: m is the steepness (rise over run) and b is where the line crosses the y axis.",
                        &["m = 1", "b = 0", "y = m x + b"],
                    )
                    .sliders(&[("m", -5.0, 5.0), ("b", -5.0, 5.0)]),
                    entry(
                        "Point-slope form",
                        "A line through the point (p, q) with slope m. Drag the point and the line follows.",
                        &["m = 2", "p = 1", "q = 2", "y = m (x - p) + q", "(p, q)"],
                    )
                    .sliders(&[("m", -5.0, 5.0), ("p", -8.0, 8.0), ("q", -5.0, 5.0)]),
                    entry(
                        "Parallel and perpendicular",
                        "Parallel lines share a slope m. A perpendicular line has slope −1/m, so the slopes multiply to −1.",
                        &["m = 0.5", "y = m x + 1", "y = m x - 2", "y = -x/m"],
                    )
                    .sliders(&[("m", 0.2, 4.0)]),
                    entry(
                        "Distance between points",
                        "The distance from (a, b) to (c, d) is √((c − a)² + (d − b)²), Pythagoras in disguise. The circle shows every point at that distance.",
                        &[
                            "a = -2",
                            "b = -1",
                            "c = 3",
                            "d = 2",
                            "(a, b)",
                            "(c, d)",
                            "(a + t (c - a), b + t (d - b))",
                            "(x - a)^2 + (y - b)^2 = (c - a)^2 + (d - b)^2",
                        ],
                    )
                    .t(0.0, 1.0),
                    entry(
                        "Linear inequality",
                        "y > 2x − 1 shades everything above the boundary line.",
                        &["y > 2x - 1", "y = 2x - 1"],
                    ),
                ],
            ),
            folder(
                "Quadratics",
                "Parabolas, roots and the quadratic formula.",
                &[
                    entry(
                        "Vertex form",
                        "y = a(x − h)² + k has its turning point at (h, k). a stretches it, and flips it when negative.",
                        &["a = 1", "h = 0", "k = 0", "y = a (x - h)^2 + k", "(h, k)"],
                    )
                    .sliders(&[("a", -3.0, 3.0), ("h", -6.0, 6.0), ("k", -5.0, 5.0)]),
                    entry(
                        "Quadratic formula",
                        "The dots are x = (−b ± √(b² − 4ac)) / 2a. When the discriminant b² − 4ac turns negative, the roots vanish: the parabola misses the axis.",
                        &[
                            "a = 1",
                            "b = -1",
                            "c = -2",
                            "y = a x^2 + b x + c",
                            "((-b + sqrt(b^2 - 4a c))/(2a), 0)",
                            "((-b - sqrt(b^2 - 4a c))/(2a), 0)",
                        ],
                    )
                    .sliders(&[("a", -3.0, 3.0), ("b", -6.0, 6.0), ("c", -6.0, 6.0)]),
                    entry(
                        "Completing the square",
                        "x² + bx and (x + b/2)² − b²/4 are the same parabola, so the second form shows the vertex at once.",
                        &["b = 3", "y = x^2 + b x", "y = (x + b/2)^2 - b^2/4"],
                    )
                    .sliders(&[("b", -6.0, 6.0)]),
                    entry(
                        "Focus and directrix",
                        "Every point of the parabola is as far from the focus (0, p) as from the line y = −p. Satellite dishes use this.",
                        &["p = 1", "y = x^2/(4p)", "(0, p)", "y = -p"],
                    )
                    .sliders(&[("p", 0.2, 4.0)]),
                ],
            ),
            folder(
                "Polynomials & Rational",
                "Higher powers, factors and fractions of polynomials.",
                &[
                    entry(
                        "Factored cubic",
                        "y = (x − p)(x − q)(x − s) crosses zero exactly at p, q and s. Move a root and watch the curve bend.",
                        &[
                            "p = -3",
                            "q = 0",
                            "s = 2",
                            "y = (x - p)(x - q)(x - s)/4",
                            "(p, 0)",
                            "(q, 0)",
                            "(s, 0)",
                        ],
                    )
                    .sliders(&[("p", -6.0, 6.0), ("q", -6.0, 6.0), ("s", -6.0, 6.0)]),
                    entry(
                        "End behaviour",
                        "Even powers rise on both sides; odd powers go opposite ways. The highest power wins far from the origin.",
                        &["y = x^2", "y = x^3", "y = x^4", "y = x^5"],
                    )
                    .window(-2.0, 2.0, -4.0, 4.0),
                    entry(
                        "Multiplicity",
                        "A squared factor (x + 2)² makes the curve touch and bounce; a cubed factor (x − 1)³ flattens as it crosses.",
                        &["y = (x + 2)^2 (x - 1)^3/10"],
                    )
                    .window(-4.0, 3.0, -4.0, 4.0),
                    entry(
                        "Rational asymptotes",
                        "y = 1/(x − a) + b never reaches the vertical line x = a or the horizontal line y = b.",
                        &["a = 1", "b = 1", "y = 1/(x - a) + b", "x = a", "y = b"],
                    )
                    .sliders(&[("a", -5.0, 5.0), ("b", -4.0, 4.0)]),
                    entry(
                        "Slant asymptote",
                        "When the top has one more power than the bottom, the graph hugs a slanted line, here y = x + 1.",
                        &["y = (x^2 + 1)/(x - 1)", "y = x + 1"],
                    ),
                    entry(
                        "Binomial theorem",
                        "(x + 1)⁵ expands into Σ C(5, k) xᵏ: the binomial coefficients 1, 5, 10, 10, 5, 1. Both draw the same curve.",
                        &["y = (x + 1)^5", "y = sum(k, 0, 5, binom(5, k) x^k)"],
                    )
                    .window(-3.0, 1.0, -4.0, 8.0),
                    entry(
                        "Absolute value",
                        "|x| makes a V. a changes the steepness, (h, k) moves the corner.",
                        &["a = 1", "h = 0", "k = 0", "y = a abs(x - h) + k"],
                    )
                    .sliders(&[("a", -3.0, 3.0), ("h", -6.0, 6.0), ("k", -5.0, 5.0)]),
                    entry(
                        "Roots and powers",
                        "√x undoes x² (for x ≥ 0) and ∛x undoes x³, so each pair is a mirror image across y = x.",
                        &["y = x^2", "y = sqrt(x)", "y = x^3", "y = cbrt(x)", "y = x"],
                    )
                    .window(-3.0, 3.0, -2.0, 2.0),
                ],
            ),
            folder(
                "Systems & Inequalities",
                "Where graphs meet, and regions that satisfy conditions.",
                &[
                    entry(
                        "Two lines meet",
                        "A system of two linear equations has one solution where the lines cross: here (2, 3).",
                        &["y = 2x - 1", "y = -x + 5", "(2, 3)"],
                    ),
                    entry(
                        "Line meets parabola",
                        "Depending on the slope m, a line through the origin meets y = x² − 2 twice: there is always a real solution.",
                        &["m = 1", "y = x^2 - 2", "y = m x"],
                    )
                    .sliders(&[("m", -6.0, 6.0)]),
                    entry(
                        "Line meets circle",
                        "A line can cut a circle twice, touch it once (a tangent), or miss it. Slide b to find the tangent.",
                        &["m = 0.5", "b = 2", "x^2 + y^2 = 9", "y = m x + b"],
                    )
                    .sliders(&[("m", -4.0, 4.0), ("b", -6.0, 6.0)]),
                    entry(
                        "System of inequalities",
                        "min(A, B) > 0 means A > 0 and B > 0. The region is above y = x² − 4 and below y = 2.",
                        &["min(y - x^2 + 4, 2 - y) > 0", "y = x^2 - 4", "y = 2"],
                    ),
                    entry(
                        "Linear programming",
                        "The shaded polygon is every plan meeting the constraints. Slide the profit line c: the best plan is always a corner.",
                        &[
                            "c = 4",
                            "min(min(x, y), min(8 - x - y, 12 - 3x - y)) > 0",
                            "y = -x/2 + c",
                        ],
                    )
                    .sliders(&[("c", 0.0, 10.0)])
                    .window(-1.0, 10.0, -1.0, 10.0),
                ],
            ),
        ],
    ),
    folder(
        "Functions",
        "Inputs, outputs and the shapes functions make.",
        &[
            entry(
                "Transformations",
                "a·sin(b(x − h)) + k: a stretches vertically, b squeezes horizontally, h shifts sideways, k shifts up.",
                &["a = 1", "b = 1", "h = 0", "k = 0", "y = a sin(b (x - h)) + k", "y = sin(x)"],
            )
            .sliders(&[("a", -4.0, 4.0), ("b", 0.1, 4.0), ("h", -4.0, 4.0), ("k", -4.0, 4.0)]),
            entry(
                "Parent functions",
                "The basic shapes most functions are built from: line, parabola, cubic, root, reciprocal and absolute value.",
                &["y = x", "y = x^2", "y = x^3", "y = sqrt(x)", "y = 1/x", "y = abs(x)"],
            )
            .window(-4.0, 4.0, -3.0, 3.0),
            entry(
                "Composition",
                "Order matters: sin(x²) squares first, then takes the sine; sin(x)² takes the sine, then squares.",
                &["y = sin(x^2)", "y = sin(x)^2"],
            )
            .window(-4.0, 4.0, -1.5, 1.5),
            entry(
                "Inverse functions",
                "eˣ and ln(x) undo each other, so their graphs mirror across the line y = x.",
                &["y = e^x", "y = ln(x)", "y = x"],
            )
            .window(-4.0, 6.0, -3.0, 5.0),
            entry(
                "Inverse by reflection",
                "Swap x and y to get an inverse: x = y³ + y is the inverse of y = x³ + x, even with no formula for it.",
                &["y = x^3 + x", "x = y^3 + y", "y = x"],
            )
            .window(-4.0, 4.0, -3.0, 3.0),
            entry(
                "Piecewise function",
                "Different rules on different parts: −x for x < 0 and x² for x > 0, switched by the sign of x.",
                &["y = (1 - sign(x))/2 (-x) + (1 + sign(x))/2 x^2"],
            )
            .window(-4.0, 4.0, -1.0, 5.0),
            entry(
                "Even and odd",
                "Even functions mirror across the y axis (f(−x) = f(x)); odd functions have half-turn symmetry about the origin (f(−x) = −f(x)).",
                &["y = x^4 - 3x^2", "y = x^3 - 3x"],
            )
            .window(-3.0, 3.0, -4.0, 4.0),
            entry(
                "Exponential growth and decay",
                "y = P·e^(gx) grows when g > 0 and decays when g < 0. Compound interest, bacteria and radioactivity all follow it.",
                &["P = 1", "g = 0.5", "y = P e^(g x)"],
            )
            .sliders(&[("P", 0.1, 5.0), ("g", -2.0, 2.0)])
            .window(-5.0, 5.0, -1.0, 8.0),
            entry(
                "Logarithm bases",
                "log_B(x) = ln(x) / ln(B). Every logarithm passes through (1, 0); bigger bases grow more slowly.",
                &["B = 2", "y = ln(x)/ln(B)", "y = ln(x)", "y = log10(x)"],
            )
            .sliders(&[("B", 1.1, 10.0)])
            .window(-1.0, 10.0, -3.0, 4.0),
            entry(
                "Periodic functions",
                "Patterns that repeat: a sawtooth, a triangle wave and a square wave.",
                &["y = mod(x, 2)", "y = abs(mod(x, 2) - 1) - 2", "y = sign(sin(pi x)) + 3"],
            )
            .window(-5.0, 5.0, -3.0, 5.0),
            entry(
                "Neural network activations",
                "The switches inside neural networks: sigmoid, tanh, ReLU max(x, 0) and softplus ln(1 + eˣ).",
                &["y = 1/(1 + e^(-x))", "y = tanh(x)", "y = max(x, 0)", "y = ln(1 + e^x)"],
            )
            .window(-5.0, 5.0, -1.5, 4.0),
        ],
    ),
    folder(
        "Trigonometry",
        "Angles, circles and waves.",
        &[
            entry(
                "Unit circle",
                "For angle a, the point on the unit circle is (cos a, sin a). The vertical drop is the sine, the horizontal the cosine.",
                &[
                    "a = 0.8",
                    "x^2 + y^2 = 1",
                    "(cos(a), sin(a))",
                    "(t cos(a), t sin(a))",
                    "(cos(a), t sin(a))",
                    "(t cos(a), 0)",
                ],
            )
            .sliders(&[("a", 0.0, TAU)])
            .t(0.0, 1.0)
            .window(-2.0, 2.0, -1.3, 1.3),
            entry(
                "Circle to sine wave",
                "A point going round a circle traces a sine wave when its height is plotted against the angle.",
                &[
                    "(x + 2)^2 + y^2 = 1",
                    "y = sin(x)",
                    "(-2 + cos(time), sin(time))",
                    "(mod(time, 2pi), sin(time))",
                    "(-2 + cos(time) + t (mod(time, 2pi) + 2 - cos(time)), sin(time))",
                ],
            )
            .t(0.0, 1.0)
            .window(-3.5, 7.0, -2.0, 2.0)
            .animate(),
            entry(
                "Amplitude, period, phase",
                "A·sin(Bx + C) + D: A is the height, 2π/B the period, C slides the wave and D lifts it.",
                &["A = 2", "B = 1", "C = 0", "D = 0", "y = A sin(B x + C) + D"],
            )
            .sliders(&[("A", -4.0, 4.0), ("B", 0.1, 5.0), ("C", -PI, PI), ("D", -3.0, 3.0)]),
            entry(
                "Pythagorean identity",
                "sin²x + cos²x = 1 for every x: the two waves fill each other's gaps exactly.",
                &["y = sin(x)^2", "y = cos(x)^2", "y = sin(x)^2 + cos(x)^2"],
            )
            .window(-7.0, 7.0, -0.5, 1.5),
            entry(
                "Double angle",
                "sin(2x) = 2 sin(x) cos(x): two formulas, one curve.",
                &["y = sin(2x)", "y = 2 sin(x) cos(x)"],
            ),
            entry(
                "Tangent and its asymptotes",
                "tan x = sin x / cos x shoots off to infinity wherever cos x = 0, at ±π/2, ±3π/2, …",
                &["y = tan(x)", "x = pi/2", "x = -pi/2"],
            ),
            entry(
                "Reciprocal functions",
                "sec, csc and cot are 1/cos, 1/sin and 1/tan: they blow up where the originals cross zero.",
                &["y = sec(x)", "y = csc(x)", "y = cot(x)"],
            ),
            entry(
                "Inverse trig",
                "arcsin, arccos and arctan give back the angle. arctan flattens toward ±π/2.",
                &["y = asin(x)", "y = acos(x)", "y = atan(x)"],
            )
            .window(-4.0, 4.0, -2.0, 3.5),
            entry(
                "Adding waves",
                "sin(x) + sin(3x)/3: adding a smaller, faster wave squares off the shape, the first step of a Fourier series.",
                &["y = sin(x)", "y = sin(3x)/3", "y = sin(x) + sin(3x)/3"],
            )
            .window(-7.0, 7.0, -1.5, 1.5),
            entry(
                "Hyperbolic functions",
                "cosh and sinh are to the hyperbola x² − y² = 1 what cos and sin are to the circle: (cosh a, sinh a) lies on it.",
                &[
                    "a = 1",
                    "y = sinh(x)",
                    "y = cosh(x)",
                    "y = tanh(x)",
                    "x^2 - y^2 = 1",
                    "(cosh(a), sinh(a))",
                ],
            )
            .sliders(&[("a", -2.5, 2.5)])
            .window(-5.0, 5.0, -4.0, 4.0),
        ],
    ),
    parent(
        "Geometry",
        "Shapes, theorems and patterns in the plane.",
        &[
            folder(
                "Euclidean",
                "Circles, triangles and polygons.",
                &[
                    entry(
                        "Circle equation",
                        "(x − h)² + (y − k)² = R²: every point at distance R from the centre (h, k).",
                        &["h = 0", "k = 0", "R = 3", "(x - h)^2 + (y - k)^2 = R^2", "(h, k)"],
                    )
                    .sliders(&[("h", -6.0, 6.0), ("k", -4.0, 4.0), ("R", 0.5, 6.0)]),
                    entry(
                        "Thales' theorem",
                        "Any triangle drawn on a diameter with its third corner on the circle has a right angle at that corner.",
                        &[
                            "s = 1",
                            "x^2 + y^2 = 1",
                            "(cos(s), sin(s))",
                            "(-1 + t (cos(s) + 1), t sin(s))",
                            "(1 + t (cos(s) - 1), t sin(s))",
                            "(2t - 1, 0)",
                        ],
                    )
                    .sliders(&[("s", 0.0, TAU)])
                    .t(0.0, 1.0)
                    .window(-2.0, 2.0, -1.3, 1.3),
                    entry(
                        "Regular polygons",
                        "A polygon with m equal sides drawn in polar form. Non-whole m makes strange broken polygons.",
                        &["m = 6", "r = cos(pi/m)/cos(mod(θ, 2pi/m) - pi/m)", "x^2 + y^2 = 1"],
                    )
                    .sliders(&[("m", 3.0, 12.0)])
                    .window(-2.0, 2.0, -1.3, 1.3),
                    entry(
                        "Triangle centroid",
                        "The centroid, the average of the three corners, is the balance point. Drag corner C around.",
                        &[
                            "cx = 1",
                            "cy = 3",
                            "(0, 0)",
                            "(4, 0)",
                            "(cx, cy)",
                            "((4 + cx)/3, cy/3)",
                            "(4t, 0)",
                            "(t cx, t cy)",
                            "(4 + t (cx - 4), t cy)",
                        ],
                    )
                    .sliders(&[("cx", -3.0, 7.0), ("cy", -4.0, 5.0)])
                    .t(0.0, 1.0)
                    .window(-4.0, 8.0, -4.0, 5.0),
                    entry(
                        "Spiral of Theodorus",
                        "Right triangles glued together, each with an outer side of 1. The hypotenuses are √2, √3, √4, …",
                        &[
                            "(sqrt(n + 1) cos(sum(k, 1, n, atan(1/sqrt(k)))), sqrt(n + 1) sin(sum(k, 1, n, atan(1/sqrt(k)))))",
                        ],
                    )
                    .t(0.0, 16.0)
                    .window(-6.0, 6.0, -4.0, 4.0),
                    entry(
                        "Ellipse foci",
                        "An ellipse is every point whose distances to the two foci add up to the same total (2a).",
                        &[
                            "a = 4",
                            "b = 2.5",
                            "x^2/a^2 + y^2/b^2 = 1",
                            "(sqrt(a^2 - b^2), 0)",
                            "(-sqrt(a^2 - b^2), 0)",
                        ],
                    )
                    .sliders(&[("a", 1.0, 8.0), ("b", 0.5, 5.0)]),
                ],
            ),
            folder(
                "Tilings & Patterns",
                "Repeating patterns made with mod and floor.",
                &[
                    entry(
                        "Checkerboard",
                        "floor(x) + floor(y) is even on half the squares: a chessboard from one inequality.",
                        &["mod(floor(x) + floor(y), 2) < 1"],
                    )
                    .window(-8.0, 8.0, -5.0, 5.0),
                    entry(
                        "Circle packing",
                        "mod repeats one circle in every 2 × 2 cell.",
                        &["hypot(mod(x, 2) - 1, mod(y, 2) - 1) < 1"],
                    )
                    .window(-8.0, 8.0, -5.0, 5.0),
                    entry(
                        "Truchet arcs",
                        "Quarter circles in every tile join into endless curving paths.",
                        &[
                            "abs(hypot(mod(x, 1), mod(y, 1)) - 0.5) < 0.08",
                            "abs(hypot(mod(x, 1) - 1, mod(y, 1) - 1) - 0.5) < 0.08",
                        ],
                    )
                    .window(-4.0, 4.0, -2.5, 2.5),
                    entry(
                        "Concentric rings",
                        "Shading where x² + y² mod 4 < 2 gives rings that get thinner further out, since the area between rings stays the same.",
                        &["mod(x^2 + y^2, 4) < 2"],
                    )
                    .window(-8.0, 8.0, -5.0, 5.0),
                ],
            ),
        ],
    ),
    folder(
        "Conic Sections",
        "The curves you get by slicing a cone: circle, ellipse, parabola, hyperbola.",
        &[
            entry(
                "Ellipse",
                "x²/a² + y²/b² = 1, a stretched circle with two foci.",
                &["a = 4", "b = 2", "x^2/a^2 + y^2/b^2 = 1"],
            )
            .sliders(&[("a", 0.5, 8.0), ("b", 0.5, 5.0)]),
            entry(
                "Parabola",
                "y² = 4px: every point is equally far from the focus (p, 0) and the directrix x = −p.",
                &["p = 1", "y^2 = 4p x", "(p, 0)", "x = -p"],
            )
            .sliders(&[("p", 0.2, 4.0)]),
            entry(
                "Hyperbola",
                "x²/a² − y²/b² = 1: two branches that approach the asymptotes y = ±(b/a)x.",
                &["a = 2", "b = 1.5", "x^2/a^2 - y^2/b^2 = 1", "y = b x/a", "y = -b x/a"],
            )
            .sliders(&[("a", 0.5, 5.0), ("b", 0.5, 5.0)]),
            entry(
                "Eccentricity",
                "One polar formula draws every conic: a circle at 0, ellipses below 1, a parabola at 1, hyperbolas above.",
                &["ecc = 0.5", "r = 2/(1 + ecc cos(θ))"],
            )
            .sliders(&[("ecc", 0.0, 2.0)]),
            entry(
                "General conic",
                "Ax² + Bxy + Cy² + Dx + Ey = 9 covers every conic. The Bxy term rotates it.",
                &["A = 1", "B = 0", "C = 1", "D = 0", "E = 0", "A x^2 + B x y + C y^2 + D x + E y = 9"],
            )
            .sliders(&[("A", -3.0, 3.0), ("B", -3.0, 3.0), ("C", -3.0, 3.0), ("D", -5.0, 5.0), ("E", -5.0, 5.0)]),
            entry(
                "Slicing a cone",
                "A plane cutting a double cone. Tilt it with m: the cut is an ellipse, then a parabola, then a hyperbola.",
                &["m = 0.5", "c = 1", "x^2 + y^2 = z^2", "z = m x + c"],
            )
            .sliders(&[("m", -2.0, 2.0), ("c", -2.0, 2.0)])
            .cube(4.0),
        ],
    ),
    folder(
        "Polar Curves",
        "Curves drawn by distance r at each angle θ.",
        &[
            entry(
                "Cardioid",
                "r = a(1 + cos θ), the heart-shaped curve traced by a point on a circle rolling around another circle of the same size.",
                &["a = 2", "r = a (1 + cos(θ))"],
            )
            .sliders(&[("a", 0.5, 4.0)]),
            entry(
                "Limaçon",
                "r = b + 2cos θ. Below b = 2 it grows an inner loop; at 2 it is a cardioid.",
                &["b = 1", "r = b + 2cos(θ)"],
            )
            .sliders(&[("b", 0.0, 4.0)]),
            entry(
                "Rose curves",
                "r = 4cos(kθ). Odd k gives k petals, even k gives 2k. Try fractions like 2.5.",
                &["k = 5", "r = 4cos(k θ)"],
            )
            .sliders(&[("k", 0.5, 12.0)])
            .t(0.0, 4.0 * PI),
            entry(
                "Lemniscate",
                "r² = 16cos(2θ), the infinity sign. Where cos(2θ) is negative there is no curve.",
                &["r = 4sqrt(cos(2θ))", "r = -4sqrt(cos(2θ))"],
            ),
            entry(
                "Archimedean spiral",
                "r = θ/2: the gap between turns never changes, like a rolled-up carpet.",
                &["r = θ/2"],
            )
            .t(0.0, 8.0 * PI)
            .window(-14.0, 14.0, -9.0, 9.0),
            entry(
                "Logarithmic spiral",
                "r = e^(0.15θ) keeps the same shape at every scale, as in galaxies, hurricanes and shells.",
                &["r = e^(0.15θ)"],
            )
            .t(-10.0, 14.0),
            entry(
                "Fermat's spiral",
                "r = ±√θ: turns get closer as the spiral grows, so every ring encloses the same area.",
                &["r = sqrt(θ)", "r = -sqrt(θ)"],
            )
            .t(0.0, 8.0 * PI)
            .window(-6.0, 6.0, -4.0, 4.0),
            entry(
                "Hyperbolic spiral",
                "r = 6/θ winds in forever toward the centre without reaching it.",
                &["r = 6/θ"],
            )
            .t(0.3, 30.0),
            entry(
                "Polar flower",
                "Small ripples added to a circle make petals, with finer ripples on top.",
                &["r = 3 + sin(8θ) + 0.5cos(24θ)"],
            ),
            entry(
                "Cochleoid",
                "r = 4 sin(θ)/θ, a snail-shell curve.",
                &["r = 4sin(θ)/θ"],
            )
            .t(-20.0, 20.0)
            .window(-3.0, 5.0, -2.5, 2.5),
            entry(
                "Circles in polar",
                "r = 3 is a circle at the origin; r = 4cos θ and r = 4sin θ are circles through it.",
                &["r = 3", "r = 4cos(θ)", "r = 4sin(θ)"],
            ),
            entry(
                "Area inside a polar curve",
                "Shading every point closer to the origin than the cardioid: the area is ∫ ½r² dθ = 6π.",
                &["hypot(x, y) < 2(1 + cos(atan2(y, x)))", "r = 2(1 + cos(θ))"],
            ),
        ],
    ),
    folder(
        "Parametric Curves",
        "Curves where x and y both follow a parameter t.",
        &[
            entry(
                "Lissajous figures",
                "Two perpendicular vibrations with frequencies a and b. Oscilloscopes draw these; the ratio a:b sets the knot.",
                &["a = 3", "b = 2", "p = 1.5708", "(4sin(a t + p), 4sin(b t))"],
            )
            .sliders(&[("a", 1.0, 8.0), ("b", 1.0, 8.0), ("p", 0.0, PI)]),
            entry(
                "Cycloid",
                "The path of a point on a rolling wheel. A ball rolls down this curve faster than down any other.",
                &[
                    "(t - sin(t), 1 - cos(t))",
                    "(mod(time, 4pi) + cos(t), 1 + sin(t))",
                    "(mod(time, 4pi) - sin(mod(time, 4pi)), 1 - cos(mod(time, 4pi)))",
                ],
            )
            .t(0.0, 4.0 * PI)
            .window(-1.0, 13.5, -1.5, 3.5)
            .animate(),
            entry(
                "Epicycloid",
                "A circle of radius b rolling around the outside of a circle of radius a. Whole-number ratios give a/b cusps.",
                &[
                    "a = 3",
                    "b = 1",
                    "((a + b) cos(t) - b cos((a + b)/b t), (a + b) sin(t) - b sin((a + b)/b t))",
                ],
            )
            .sliders(&[("a", 1.0, 8.0), ("b", 0.2, 3.0)]),
            entry(
                "Hypocycloid",
                "A circle rolling inside another. a/b = 4 gives the four-pointed astroid, 3 the deltoid.",
                &[
                    "a = 4",
                    "b = 1",
                    "((a - b) cos(t) + b cos((a - b)/b t), (a - b) sin(t) - b sin((a - b)/b t))",
                ],
            )
            .sliders(&[("a", 1.0, 8.0), ("b", 0.2, 3.0)]),
            entry(
                "Spirograph",
                "A pen at distance d from the centre of a wheel of radius q rolling inside a ring of radius R: the toy's hypotrochoids.",
                &[
                    "R = 5",
                    "q = 3",
                    "d = 5",
                    "((R - q) cos(t) + d cos((R - q)/q t), (R - q) sin(t) - d sin((R - q)/q t))",
                ],
            )
            .sliders(&[("R", 1.0, 10.0), ("q", 0.5, 8.0), ("d", 0.0, 8.0)])
            .t(0.0, 6.0 * PI)
            .window(-12.0, 12.0, -8.0, 8.0),
            entry(
                "Involute of a circle",
                "Unwinding a taut string from a circle traces this spiral. Gear teeth are shaped like it.",
                &["(cos(t) + t sin(t), sin(t) - t cos(t))", "x^2 + y^2 = 1"],
            )
            .t(0.0, 4.0 * PI)
            .window(-14.0, 14.0, -9.0, 9.0),
            entry(
                "Butterfly curve",
                "Temple Fay's butterfly, built from exponentials and sines.",
                &[
                    "(sin(t) (e^cos(t) - 2cos(4t) - sin(t/12)^5), cos(t) (e^cos(t) - 2cos(4t) - sin(t/12)^5))",
                ],
            )
            .t(0.0, 12.0 * PI)
            .window(-6.0, 6.0, -3.0, 4.5),
            entry(
                "Heart curve",
                "A heart drawn with sines and cosines of t, 2t, 3t and 4t.",
                &["(16sin(t)^3, 13cos(t) - 5cos(2t) - 2cos(3t) - cos(4t))"],
            )
            .window(-24.0, 24.0, -18.0, 14.0),
            entry(
                "Quadratic Bézier",
                "Font outlines use these curves. The curve starts toward the control point and bends away: drag (px, py).",
                &[
                    "px = 0",
                    "py = 5",
                    "((1 - t)^2 (-4) + 2(1 - t) t px + 4t^2, (1 - t)^2 (-2) + 2(1 - t) t py - 2t^2)",
                    "(-4 + t (px + 4), -2 + t (py + 2))",
                    "(px + t (4 - px), py + t (-2 - py))",
                    "(-4, -2)",
                    "(px, py)",
                    "(4, -2)",
                ],
            )
            .sliders(&[("px", -8.0, 8.0), ("py", -5.0, 6.0)])
            .t(0.0, 1.0),
            entry(
                "Cubic Bézier",
                "Two control points shape a cubic Bézier, the curve behind vector drawing tools and animation easing.",
                &[
                    "ax = -3",
                    "ay = 5",
                    "bx = 3",
                    "by = -5",
                    "((1 - t)^3 (-5) + 3(1 - t)^2 t ax + 3(1 - t) t^2 bx + 5t^3, 3(1 - t)^2 t ay + 3(1 - t) t^2 by)",
                    "(-5, 0)",
                    "(ax, ay)",
                    "(bx, by)",
                    "(5, 0)",
                ],
            )
            .sliders(&[("ax", -8.0, 8.0), ("ay", -6.0, 6.0), ("bx", -8.0, 8.0), ("by", -6.0, 6.0)])
            .t(0.0, 1.0),
        ],
    ),
    parent(
        "Calculus",
        "Change, accumulation and infinite sums.",
        &[
            folder(
                "Limits",
                "What a function approaches.",
                &[
                    entry(
                        "sin(x)/x",
                        "Undefined at 0, yet the graph heads straight for 1. This limit is why the derivative of sin is cos.",
                        &["y = sin(x)/x", "(0, 1)"],
                    )
                    .window(-15.0, 15.0, -0.5, 1.3),
                    entry(
                        "Squeeze theorem",
                        "x² sin(1/x) wiggles infinitely often near 0 but is trapped between ±x², so its limit is 0.",
                        &["y = x^2 sin(1/x)", "y = x^2", "y = -x^2"],
                    )
                    .window(-0.5, 0.5, -0.25, 0.25),
                    entry(
                        "Epsilon–delta",
                        "For any band of height ε around the limit, some window of width δ keeps the graph inside it. That is the definition of a limit.",
                        &[
                            "a = 1",
                            "eps = 0.5",
                            "delta = 0.2",
                            "y = x^2",
                            "abs(y - a^2) < eps",
                            "abs(x - a) < delta",
                        ],
                    )
                    .sliders(&[("a", 0.0, 2.0), ("eps", 0.05, 1.5), ("delta", 0.01, 1.0)])
                    .window(-1.0, 3.0, -1.0, 4.0),
                    entry(
                        "Removable discontinuity",
                        "(x² − 1)/(x − 1) equals x + 1 everywhere except x = 1, where it is undefined: a hole at (1, 2).",
                        &["y = (x^2 - 1)/(x - 1)", "(1, 2)"],
                    )
                    .window(-3.0, 4.0, -2.0, 5.0),
                    entry(
                        "Jump discontinuity",
                        "The sign function jumps from −1 to 1. The left and right limits disagree, so there is no limit at 0.",
                        &["y = sign(x)"],
                    )
                    .window(-3.0, 3.0, -2.0, 2.0),
                    entry(
                        "Limits at infinity",
                        "Far out, only the highest powers matter: (2x² + 1)/(x² + 1) → 2.",
                        &["y = (2x^2 + 1)/(x^2 + 1)", "y = 2"],
                    )
                    .window(-20.0, 20.0, -0.5, 3.0),
                    entry(
                        "The number e",
                        "(1 + 1/x)^x, interest compounded ever more often, climbs to e = 2.718…",
                        &["y = (1 + 1/x)^x", "y = e"],
                    )
                    .window(0.0, 50.0, 0.0, 3.5),
                ],
            ),
            folder(
                "Derivatives",
                "Slopes, tangents and rates of change.",
                &[
                    entry(
                        "Tangent line",
                        "The tangent at x = a has slope f′(a) = a² − 1. It is flat at the peaks and valleys.",
                        &[
                            "a = 1.5",
                            "y = x^3/3 - x",
                            "(a, a^3/3 - a)",
                            "y = (a^2 - 1)(x - a) + a^3/3 - a",
                        ],
                    )
                    .sliders(&[("a", -3.0, 3.0)])
                    .window(-4.0, 4.0, -3.0, 3.0),
                    entry(
                        "Secant to tangent",
                        "Shrink h and the line through two points on the curve turns into the tangent: the derivative is a limit.",
                        &[
                            "a = 1",
                            "h = 2",
                            "y = x^2/2",
                            "(a, a^2/2)",
                            "(a + h, (a + h)^2/2)",
                            "y = ((a + h)^2/2 - a^2/2)/h (x - a) + a^2/2",
                        ],
                    )
                    .sliders(&[("a", -3.0, 3.0), ("h", 0.01, 4.0)])
                    .window(-4.0, 5.0, -1.0, 7.0),
                    entry(
                        "f, f′ and f″",
                        "Where f peaks, f′ crosses zero. Where f bends (inflection), f″ crosses zero.",
                        &["y = x^4/4 - x^2", "y = x^3 - 2x", "y = 3x^2 - 2"],
                    )
                    .window(-3.0, 3.0, -3.0, 4.0),
                    entry(
                        "Mean value theorem",
                        "Somewhere between the ends, the tangent is parallel to the line joining them. Here at x = 2/√3.",
                        &[
                            "y = x^3 - x",
                            "y = 3x",
                            "y = 3(x - 2/sqrt(3)) + (2/sqrt(3))^3 - 2/sqrt(3)",
                            "(0, 0)",
                            "(2, 6)",
                        ],
                    )
                    .window(-1.0, 3.0, -2.0, 7.0),
                    entry(
                        "Open box",
                        "Cut squares of side x from a 10 × 10 sheet and fold it: the volume x(10 − 2x)² peaks at x = 5/3.",
                        &["y = x (10 - 2x)^2", "(5/3, 5/3 (10 - 10/3)^2)"],
                    )
                    .window(-0.5, 5.5, -10.0, 90.0),
                    entry(
                        "Linear approximation",
                        "Near x = a the tangent line is a good stand-in for the curve, as in √4.1 ≈ 2.025.",
                        &["a = 4", "y = sqrt(x)", "y = sqrt(a) + (x - a)/(2sqrt(a))"],
                    )
                    .sliders(&[("a", 0.2, 9.0)])
                    .window(-1.0, 10.0, -1.0, 4.0),
                    entry(
                        "Implicit differentiation",
                        "On x² + y² = 25, 2x + 2y·y′ = 0 gives slope −x/y = −3/4 at (3, 4).",
                        &["x^2 + y^2 = 25", "(3, 4)", "y = -3/4 (x - 3) + 4"],
                    ),
                ],
            ),
            folder(
                "Integrals",
                "Areas and accumulation.",
                &[
                    entry(
                        "Area under a curve",
                        "The definite integral from a to b is the shaded area between the curve and the axis.",
                        &["a = 0", "b = 3", "y = sin(x) + 1.5", "min(min(y, sin(x) + 1.5 - y), min(x - a, b - x)) > 0"],
                    )
                    .sliders(&[("a", -6.0, 6.0), ("b", -6.0, 8.0)]),
                    entry(
                        "Riemann sums",
                        "Rectangles of width 1/N. As N grows, their total area approaches the integral.",
                        &[
                            "N = 2",
                            "y = x^2/4 + 1",
                            "min(min(y, (floor(x N)/N)^2/4 + 1 - y), min(x, 4 - x)) > 0",
                        ],
                    )
                    .sliders(&[("N", 1.0, 20.0)])
                    .window(-1.0, 5.0, -1.0, 6.0),
                    entry(
                        "Area between curves",
                        "Between y = x and y = x² from 0 to 1 the area is ∫(x − x²)dx = 1/6.",
                        &["y = x", "y = x^2", "min(x - y, y - x^2) > 0"],
                    )
                    .window(-0.5, 1.5, -0.3, 1.3),
                    entry(
                        "Fundamental theorem",
                        "Accumulating the area under cos gives sin. The second curve is a 200-rectangle sum of cos from 0 to x.",
                        &["y = cos(x)", "y = sum(k, 1, 200, cos((k - 0.5) x/200)) x/200"],
                    ),
                    entry(
                        "Gaussian integral",
                        "The area under e^(−x²) builds the error function: (√π/2) erf(x), levelling off at ±√π/2.",
                        &["y = e^(-x^2)", "y = sqrt(pi)/2 erf(x)"],
                    )
                    .window(-4.0, 4.0, -1.2, 1.4),
                    entry(
                        "Improper integral",
                        "This region goes on forever to the right, yet its area is exactly 1.",
                        &["y = 1/x^2", "min(min(y, 1/x^2 - y), x - 1) > 0"],
                    )
                    .window(0.0, 10.0, -0.2, 1.5),
                    entry(
                        "Average value",
                        "The average of sin(x) + 1 over [0, π] is 1 + 2/π: the rectangle under that height has the same area.",
                        &["y = sin(x) + 1", "y = 1 + 2/pi", "min(min(y, 1 + 2/pi - y), min(x, pi - x)) > 0"],
                    )
                    .window(-1.0, 4.5, -0.5, 2.5),
                    entry(
                        "Solid of revolution",
                        "Spinning y = √x around the x axis makes a bowl. Its volume is ∫ π(√x)² dx.",
                        &["(u, sqrt(u) cos(v), sqrt(u) sin(v))"],
                    )
                    .t(0.0, 4.0)
                    .v(0.0, TAU)
                    .cube(3.0),
                ],
            ),
            folder(
                "Series",
                "Infinite sums, polynomials that imitate functions, and waves built from waves.",
                &[
                    entry(
                        "Taylor series: sin x",
                        "Each term (−1)ᵏx^(2k+1)/(2k+1)! makes the polynomial follow sin(x) further from 0. Drag N.",
                        &["N = 3", "y = sin(x)", "y = sum(k, 0, N, (-1)^k x^(2k + 1)/(2k + 1)!)"],
                    )
                    .sliders(&[("N", 0.0, 12.0)])
                    .window(-10.0, 10.0, -4.0, 4.0),
                    entry(
                        "Taylor series: cos x",
                        "Even powers only: 1 − x²/2! + x⁴/4! − … Each N adds another hump.",
                        &["N = 3", "y = cos(x)", "y = sum(k, 0, N, (-1)^k x^(2k)/(2k)!)"],
                    )
                    .sliders(&[("N", 0.0, 12.0)])
                    .window(-10.0, 10.0, -4.0, 4.0),
                    entry(
                        "Taylor series: eˣ",
                        "eˣ = 1 + x + x²/2! + x³/3! + …, which converges for every x.",
                        &["N = 4", "y = e^x", "y = sum(k, 0, N, x^k/k!)"],
                    )
                    .sliders(&[("N", 0.0, 20.0)])
                    .window(-5.0, 4.0, -2.0, 20.0),
                    entry(
                        "Taylor series: ln(1 + x)",
                        "This series only converges for −1 < x ≤ 1. Outside, adding terms makes it worse.",
                        &["N = 5", "y = ln(1 + x)", "y = sum(k, 1, N, (-1)^(k + 1) x^k/k)"],
                    )
                    .sliders(&[("N", 1.0, 30.0)])
                    .window(-1.5, 2.5, -3.0, 2.0),
                    entry(
                        "Geometric series",
                        "1 + x + x² + … = 1/(1 − x), but only while |x| < 1.",
                        &["N = 5", "y = 1/(1 - x)", "y = sum(k, 0, N, x^k)"],
                    )
                    .sliders(&[("N", 0.0, 30.0)])
                    .window(-2.0, 2.0, -4.0, 10.0),
                    entry(
                        "Fourier: square wave",
                        "Adding odd harmonics sin((2k−1)x)/(2k−1) builds a square wave. The overshoot at the jumps never goes away (Gibbs).",
                        &["N = 5", "y = sign(sin(x))", "y = 4/pi sum(k, 1, N, sin((2k - 1) x)/(2k - 1))"],
                    )
                    .sliders(&[("N", 1.0, 50.0)])
                    .window(-10.0, 10.0, -2.0, 2.0),
                    entry(
                        "Fourier: sawtooth",
                        "Every harmonic with alternating signs builds a ramp that resets every 2π.",
                        &["N = 5", "y = mod(x + pi, 2pi) - pi", "y = 2 sum(k, 1, N, (-1)^(k + 1) sin(k x)/k)"],
                    )
                    .sliders(&[("N", 1.0, 50.0)])
                    .window(-10.0, 10.0, -4.0, 4.0),
                    entry(
                        "Fourier: triangle wave",
                        "Odd harmonics falling off as 1/k² give a triangle wave. It converges much faster than the square wave.",
                        &["N = 2", "y = 8/pi^2 sum(k, 0, N, (-1)^k sin((2k + 1) x)/(2k + 1)^2)"],
                    )
                    .sliders(&[("N", 0.0, 20.0)])
                    .window(-10.0, 10.0, -1.5, 1.5),
                    entry(
                        "Basel problem",
                        "1 + 1/4 + 1/9 + 1/16 + … creeps up to π²/6, as Euler proved in 1734.",
                        &["(n, sum(k, 1, n, 1/k^2))", "y = pi^2/6"],
                    )
                    .t(1.0, 40.0)
                    .window(0.0, 41.0, 0.8, 1.8),
                    entry(
                        "Harmonic series diverges",
                        "1 + 1/2 + 1/3 + … grows without limit, but only as fast as ln(n) + 0.577.",
                        &["(n, sum(k, 1, n, 1/k))", "y = ln(x) + 0.5772"],
                    )
                    .t(1.0, 100.0)
                    .window(0.0, 101.0, 0.0, 6.0),
                    entry(
                        "Leibniz series for π",
                        "4(1 − 1/3 + 1/5 − 1/7 + …) = π, zig-zagging around it very slowly.",
                        &["(n, 4 sum(k, 0, n, (-1)^k/(2k + 1)))", "y = pi"],
                    )
                    .t(0.0, 40.0)
                    .window(-1.0, 41.0, 2.5, 4.2),
                    entry(
                        "e as a series",
                        "1 + 1 + 1/2! + 1/3! + … reaches e = 2.71828… after only a few terms.",
                        &["(n, sum(k, 0, n, 1/k!))", "y = e"],
                    )
                    .t(0.0, 10.0)
                    .window(-1.0, 11.0, 0.5, 3.2),
                ],
            ),
        ],
    ),
    folder(
        "Multivariable Calculus",
        "Functions of x and y: surfaces, level curves and gradients.",
        &[
            entry(
                "Paraboloid",
                "z = x² + y², a bowl. Every level curve is a circle.",
                &["z = x^2 + y^2"],
            )
            .cube(3.0),
            entry(
                "Saddle",
                "z = x² − y² curves up one way and down the other. The origin is neither a maximum nor a minimum.",
                &["z = x^2 - y^2"],
            )
            .cube(3.0),
            entry(
                "Monkey saddle",
                "z = x³ − 3xy² has three dips: room for two legs and a tail.",
                &["z = x^3 - 3x y^2"],
            )
            .cube(2.0),
            entry(
                "Ripple",
                "A damped wave spreading from the centre, like a drop in a pond.",
                &["z = 2sin(2hypot(x, y))/(hypot(x, y) + 1)"],
            )
            .cube(6.0),
            entry(
                "Gaussian bump",
                "z = 3e^(−(x² + y²)/2), the bell curve in two dimensions.",
                &["z = 3e^(-(x^2 + y^2)/2)"],
            )
            .cube(4.0),
            entry(
                "Egg crate",
                "z = sin(x)sin(y), a grid of peaks and pits.",
                &["z = sin(x) sin(y)"],
            )
            .cube(6.0),
            entry(
                "Level curves",
                "Slices of z = x² + 2y² at heights 1, 4, 9 and 16, like contour lines on a map. The arrows (the gradient) cross them at right angles.",
                &["x^2 + 2y^2 = 1", "x^2 + 2y^2 = 4", "x^2 + 2y^2 = 9", "x^2 + 2y^2 = 16", "(2x, 4y)"],
            )
            .window(-6.0, 6.0, -4.0, 4.0),
            entry(
                "Lagrange multipliers",
                "Maximise xy on the circle x² + y² = 4: slide c until the curve xy = c just touches the circle. At that point the gradients line up.",
                &["c = 1", "x^2 + y^2 = 4", "x y = c", "(sqrt(2), sqrt(2))"],
            )
            .sliders(&[("c", -3.0, 3.0)])
            .window(-5.0, 5.0, -3.0, 3.0),
            entry(
                "Tangent plane",
                "The plane that just touches z = (x² + y²)/4 at (a, b), the 2D version of a tangent line.",
                &[
                    "a = 1",
                    "b = 1",
                    "z = (x^2 + y^2)/4",
                    "z = a (x - a)/2 + b (y - b)/2 + (a^2 + b^2)/4",
                    "(a, b, (a^2 + b^2)/4)",
                ],
            )
            .sliders(&[("a", -3.0, 3.0), ("b", -3.0, 3.0)])
            .cube(3.0),
            entry(
                "Partial derivative slice",
                "Hold y = b fixed and the surface becomes an ordinary curve. Its slope is the partial derivative ∂z/∂x.",
                &["b = 0.5", "z = sin(x) cos(y)", "(t, b, sin(t) cos(b))"],
            )
            .sliders(&[("b", -3.0, 3.0)])
            .t(-3.0, 3.0)
            .cube(3.0),
        ],
    ),
    folder(
        "Vector Calculus",
        "Vector fields, flow lines, curl and divergence.",
        &[
            entry(
                "Rotation",
                "(−y, x) spins around the origin. It has curl but no divergence: nothing is created or destroyed.",
                &["(-y, x)"],
            ),
            entry(
                "Source",
                "(x, y) flows out of the origin, with positive divergence everywhere.",
                &["(x, y)"],
            ),
            entry(
                "Sink",
                "(−x, −y) drains into the origin, with negative divergence.",
                &["(-x, -y)"],
            ),
            entry(
                "Saddle flow",
                "(x, −y) comes in along one axis and leaves along the other.",
                &["(x, -y)"],
            ),
            entry(
                "Shear flow",
                "(y, 0): every layer slides sideways at a speed set by its height. It has curl even though nothing turns in a circle.",
                &["(y, 0)"],
            ),
            entry(
                "Point vortex",
                "The flow of a whirlpool, faster near the centre: curl-free everywhere except at the origin.",
                &["(-y/(x^2 + y^2), x/(x^2 + y^2))"],
            ),
            entry(
                "Two vortices",
                "Two whirlpools turning the same way. Their fields simply add.",
                &[
                    "(-y/((x - 1)^2 + y^2) - y/((x + 1)^2 + y^2), (x - 1)/((x - 1)^2 + y^2) + (x + 1)/((x + 1)^2 + y^2))",
                ],
            ),
            entry(
                "Flow past a cylinder",
                "Ideal fluid flowing around a round obstacle: the streamlines part and rejoin.",
                &[
                    "(1 - (x^2 - y^2)/(x^2 + y^2)^2, -2x y/(x^2 + y^2)^2)",
                    "x^2 + y^2 = 1",
                ],
            )
            .window(-5.0, 5.0, -3.0, 3.0),
            entry(
                "Gradient field",
                "The gradient of sin(x)cos(y) points uphill, at right angles to the level curves.",
                &["(cos(x) cos(y), -sin(x) sin(y))", "sin(x) cos(y) = 0.5", "sin(x) cos(y) = -0.5"],
            ),
        ],
    ),
    parent(
        "Differential Equations",
        "Equations about rates of change, and the pictures of their solutions.",
        &[
            folder(
                "Slope Fields",
                "dy/dx = f(x, y): little slopes that solutions follow.",
                &[
                    entry(
                        "Exponential growth",
                        "dy/dx = ky: growth proportional to size. The solutions are y = C·e^(kx).",
                        &["k = 0.5", "dy/dx = k y", "y = e^(k x)"],
                    )
                    .sliders(&[("k", -1.5, 1.5)]),
                    entry(
                        "Logistic growth",
                        "dy/dx = y(1 − y/4): growth that slows as the population nears the carrying capacity 4.",
                        &["dy/dx = y (1 - y/4)", "y = 4/(1 + 3e^(-x))"],
                    )
                    .window(-2.0, 8.0, -1.0, 6.0),
                    entry(
                        "Newton's cooling",
                        "A cup of tea at 6 cools toward a room at 2. It cools fastest when the gap is largest.",
                        &["dy/dx = -0.5(y - 2)", "y = 2 + 4e^(-0.5x)"],
                    )
                    .window(-1.0, 10.0, -1.0, 7.0),
                    entry(
                        "Circles",
                        "dy/dx = −x/y is solved by circles x² + y² = C.",
                        &["dy/dx = -x/y", "x^2 + y^2 = 9"],
                    ),
                    entry(
                        "dy/dx = x − y",
                        "Every solution approaches the line y = x − 1, whatever it starts from.",
                        &["dy/dx = x - y", "y = x - 1 + 2e^(-x)", "y = x - 1"],
                    ),
                ],
            ),
            folder(
                "Phase Portraits",
                "Systems of two equations drawn as flows in the plane.",
                &[
                    entry(
                        "Harmonic oscillator",
                        "Position x and velocity y of a spring circle forever: energy is conserved.",
                        &["(y, -x)"],
                    ),
                    entry(
                        "Damped oscillator",
                        "Friction c makes the circles spiral into rest.",
                        &["c = 0.4", "(y, -x - c y)"],
                    )
                    .sliders(&[("c", 0.0, 3.0)]),
                    entry(
                        "Pendulum",
                        "Small swings make loops; with enough speed the pendulum goes over the top. The eye-shaped boundary is the separatrix.",
                        &["(y, -sin(x))"],
                    )
                    .window(-10.0, 10.0, -4.0, 4.0),
                    entry(
                        "Van der Pol oscillator",
                        "Every start settles onto the same loop (a limit cycle), as heartbeats and early radio circuits do.",
                        &["mu = 1", "(y, mu (1 - x^2) y - x)"],
                    )
                    .sliders(&[("mu", 0.0, 4.0)])
                    .window(-5.0, 5.0, -4.0, 4.0),
                    entry(
                        "Predator and prey",
                        "Rabbits x and foxes y (Lotka–Volterra). Populations chase each other round closed loops.",
                        &["a = 1", "b = 0.5", "c = 1", "(a x - b x y, b x y - c y)"],
                    )
                    .sliders(&[("a", 0.1, 3.0), ("b", 0.1, 2.0), ("c", 0.1, 3.0)])
                    .window(-0.5, 6.0, -0.5, 6.0),
                    entry(
                        "Competing species",
                        "Two species sharing food. Depending on where they start, one of them takes over.",
                        &["(x (3 - x - 2y), y (2 - x - y))"],
                    )
                    .window(-0.5, 4.0, -0.5, 3.0),
                    entry(
                        "Spiral sink",
                        "Complex eigenvalues with a negative real part: trajectories spiral into the origin.",
                        &["(-0.2x - y, x - 0.2y)"],
                    ),
                ],
            ),
        ],
    ),
    folder(
        "Linear Algebra",
        "Matrices as transformations of the plane.",
        &[
            entry(
                "Matrix transforms a circle",
                "The matrix [[a, b], [c, d]] turns the unit circle into an ellipse. Its axes are set by the singular values.",
                &["a = 2", "b = 1", "c = 0", "d = 1", "x^2 + y^2 = 1", "(a cos(t) + b sin(t), c cos(t) + d sin(t))"],
            )
            .sliders(&[("a", -3.0, 3.0), ("b", -3.0, 3.0), ("c", -3.0, 3.0), ("d", -3.0, 3.0)])
            .window(-5.0, 5.0, -3.0, 3.0),
            entry(
                "Shear",
                "[[1, s], [0, 1]] slides each row sideways in proportion to its height. The area stays the same.",
                &[
                    "s = 1",
                    "(clamp(1.5cos(t), -1, 1), clamp(1.5sin(t), -1, 1))",
                    "(clamp(1.5cos(t), -1, 1) + s clamp(1.5sin(t), -1, 1), clamp(1.5sin(t), -1, 1))",
                ],
            )
            .sliders(&[("s", -3.0, 3.0)])
            .window(-5.0, 5.0, -3.0, 3.0),
            entry(
                "Rotation matrix",
                "[[cos w, −sin w], [sin w, cos w]] turns the square by angle w without changing its shape.",
                &[
                    "w = 0.5",
                    "(clamp(1.5cos(t), -1, 1), clamp(1.5sin(t), -1, 1))",
                    "(cos(w) clamp(1.5cos(t), -1, 1) - sin(w) clamp(1.5sin(t), -1, 1), sin(w) clamp(1.5cos(t), -1, 1) + cos(w) clamp(1.5sin(t), -1, 1))",
                ],
            )
            .sliders(&[("w", 0.0, TAU)])
            .window(-3.0, 3.0, -2.0, 2.0),
            entry(
                "Determinant is area",
                "The unit square becomes a parallelogram with area |ad − bc|. When the determinant is negative, the shape is flipped.",
                &[
                    "a = 2",
                    "b = 1",
                    "c = 0.5",
                    "d = 1.5",
                    "(a (clamp(1.5cos(t), -1, 1) + 1)/2 + b (clamp(1.5sin(t), -1, 1) + 1)/2, c (clamp(1.5cos(t), -1, 1) + 1)/2 + d (clamp(1.5sin(t), -1, 1) + 1)/2)",
                    "(a, c)",
                    "(b, d)",
                ],
            )
            .sliders(&[("a", -3.0, 3.0), ("b", -3.0, 3.0), ("c", -3.0, 3.0), ("d", -3.0, 3.0)])
            .window(-4.0, 5.0, -3.0, 3.0),
            entry(
                "Eigenvectors",
                "[[2, 1], [1, 2]] stretches the circle along y = x (eigenvalue 3) and leaves y = −x at length 1 (eigenvalue 1).",
                &["x^2 + y^2 = 1", "(2cos(t) + sin(t), cos(t) + 2sin(t))", "y = x", "y = -x"],
            )
            .window(-4.0, 4.0, -2.5, 2.5),
            entry(
                "Vector addition",
                "Put the tail of v at the head of u: u + v is the diagonal of the parallelogram.",
                &[
                    "a = 3",
                    "b = 1",
                    "c = 1",
                    "d = 2",
                    "(a t, b t)",
                    "(c t, d t)",
                    "(a + c t, b + d t)",
                    "(c + a t, d + b t)",
                    "((a + c) t, (b + d) t)",
                ],
            )
            .sliders(&[("a", -4.0, 4.0), ("b", -4.0, 4.0), ("c", -4.0, 4.0), ("d", -4.0, 4.0)])
            .t(0.0, 1.0)
            .window(-5.0, 7.0, -3.0, 4.0),
            entry(
                "Solutions of a linear system",
                "Two lines meet once, never (parallel) or everywhere (the same line). Make the slopes equal and see.",
                &["m = 1", "b = 2", "y = m x + 1", "y = 2x + b"],
            )
            .sliders(&[("m", -4.0, 4.0), ("b", -4.0, 4.0)]),
        ],
    ),
    folder(
        "Complex Numbers",
        "Numbers x + iy as points in the plane.",
        &[
            entry(
                "Roots of unity",
                "The m solutions of zᵐ = 1 sit evenly around the unit circle at angles 2πn/m.",
                &["m = 7", "x^2 + y^2 = 1", "(cos(2pi n/m), sin(2pi n/m))"],
            )
            .sliders(&[("m", 2.0, 24.0)])
            .t(0.0, 23.0)
            .window(-2.0, 2.0, -1.3, 1.3),
            entry(
                "Complex exponential spiral",
                "e^((a + i)t) spirals out when a > 0 and in when a < 0. With a = 0 it is Euler's circle e^(it).",
                &["a = 0.1", "(e^(a t) cos(t), e^(a t) sin(t))"],
            )
            .sliders(&[("a", -0.3, 0.3)])
            .t(-20.0, 20.0),
            entry(
                "Squaring the plane",
                "z² maps vertical and horizontal lines to parabolas that cross at right angles: conformal maps keep angles.",
                &["(1 - t^2, 2t)", "(4 - t^2, 4t)", "(t^2 - 1, 2t)", "(t^2 - 4, 4t)"],
            )
            .t(-3.0, 3.0),
            entry(
                "Joukowski airfoil",
                "z + 1/z turns a slightly offset circle into a wing shape, which is how early aerofoils were designed.",
                &[
                    "cx = -0.1",
                    "cy = 0.1",
                    "(cx + hypot(1 - cx, cy) cos(t) + (cx + hypot(1 - cx, cy) cos(t))/((cx + hypot(1 - cx, cy) cos(t))^2 + (cy + hypot(1 - cx, cy) sin(t))^2), cy + hypot(1 - cx, cy) sin(t) - (cy + hypot(1 - cx, cy) sin(t))/((cx + hypot(1 - cx, cy) cos(t))^2 + (cy + hypot(1 - cx, cy) sin(t))^2))",
                ],
            )
            .sliders(&[("cx", -0.4, 0.0), ("cy", -0.3, 0.4)])
            .window(-3.0, 3.0, -1.5, 1.5),
            entry(
                "Real and imaginary parts of z³",
                "Re(z³) = 1 and Im(z³) = 1 are curves that always cross at right angles.",
                &["x^3 - 3x y^2 = 1", "3x^2 y - y^3 = 1"],
            )
            .window(-4.0, 4.0, -2.5, 2.5),
            entry(
                "Cassini ovals",
                "Points whose distances to ±1 multiply to c². At c = 1 it becomes the lemniscate; below, it splits in two.",
                &["c = 1.1", "((x - 1)^2 + y^2)((x + 1)^2 + y^2) = c^4"],
            )
            .sliders(&[("c", 0.3, 2.0)])
            .window(-3.0, 3.0, -1.8, 1.8),
            entry(
                "Euler's helix",
                "e^(it) moving forward in time traces a helix. Seen end-on it is a circle; from the side, cos and sin waves.",
                &["(t, cos(t), sin(t))"],
            )
            .t(-6.0, 6.0)
            .cube(6.0),
            entry(
                "Mandelbrot lemniscates",
                "Curves where |z| stays at 2 after 1, 2 and 3 steps of z → z² + c. They close in on the Mandelbrot set.",
                &[
                    "x^2 + y^2 = 4",
                    "(x^2 - y^2 + x)^2 + (2x y + y)^2 = 4",
                    "((x^2 - y^2 + x)^2 - (2x y + y)^2 + x)^2 + (2(x^2 - y^2 + x)(2x y + y) + y)^2 = 4",
                ],
            )
            .window(-2.6, 1.6, -1.4, 1.4),
        ],
    ),
    folder(
        "Probability & Statistics",
        "Chance, distributions and averages.",
        &[
            entry(
                "Normal distribution",
                "The bell curve with mean m and spread s. Heights, test scores and measurement errors look like this.",
                &["m = 0", "s = 1", "y = e^(-(x - m)^2/(2s^2))/(s sqrt(2pi))"],
            )
            .sliders(&[("m", -3.0, 3.0), ("s", 0.2, 3.0)])
            .window(-5.0, 5.0, -0.1, 0.8),
            entry(
                "68–95–99.7 rule",
                "Within k standard deviations of the mean: 68% for k = 1, 95% for 2, 99.7% for 3.",
                &["k = 1", "y = e^(-x^2/2)/sqrt(2pi)", "min(min(y, e^(-x^2/2)/sqrt(2pi) - y), k - abs(x)) > 0"],
            )
            .sliders(&[("k", 0.0, 3.5)])
            .window(-4.0, 4.0, -0.05, 0.5),
            entry(
                "Binomial distribution",
                "The chances of n successes in N tries, each with probability p. For large N it looks like a bell curve.",
                &[
                    "N = 20",
                    "p = 0.3",
                    "(n, binom(N, n) p^n (1 - p)^(N - n))",
                    "y = e^(-(x - N p)^2/(2N p (1 - p)))/sqrt(2pi N p (1 - p))",
                ],
            )
            .sliders(&[("N", 1.0, 40.0), ("p", 0.01, 0.99)])
            .t(0.0, 40.0)
            .window(-1.0, 41.0, -0.02, 0.3),
            entry(
                "Poisson distribution",
                "The chances of n events in an interval when they average L, such as emails per hour or decays per second.",
                &["L = 4", "(n, L^n e^(-L)/n!)"],
            )
            .sliders(&[("L", 0.2, 15.0)])
            .t(0.0, 25.0)
            .window(-1.0, 26.0, -0.02, 0.4),
            entry(
                "Exponential distribution",
                "Waiting times between random events. It has no memory: having waited already does not shorten the wait.",
                &["L = 1", "y = L e^(-L x) (1 + sign(x))/2"],
            )
            .sliders(&[("L", 0.1, 4.0)])
            .window(-1.0, 6.0, -0.2, 2.0),
            entry(
                "Uniform distribution and CDF",
                "Equally likely anywhere in [0, 1]. The cumulative distribution rises in a straight ramp.",
                &["y = (sign(x) - sign(x - 1))/2", "y = clamp(x, 0, 1)"],
            )
            .window(-1.0, 2.0, -0.3, 1.3),
            entry(
                "Normal CDF",
                "The area under the bell curve to the left of x, written with the error function erf.",
                &["y = (1 + erf(x/sqrt(2)))/2", "y = e^(-x^2/2)/sqrt(2pi)"],
            )
            .window(-4.0, 4.0, -0.2, 1.2),
            entry(
                "Gamma distribution",
                "Waiting for the a-th random event. The gamma function Γ(a) makes the area come out to 1.",
                &["a = 2", "y = x^(a - 1) e^(-x)/gamma(a)"],
            )
            .sliders(&[("a", 0.5, 10.0)])
            .window(-1.0, 14.0, -0.05, 0.6),
            entry(
                "Student's t vs normal",
                "With few samples (small nu) the t distribution has heavier tails. As nu grows it becomes the normal curve.",
                &[
                    "nu = 2",
                    "y = gamma((nu + 1)/2)/(sqrt(nu pi) gamma(nu/2)) (1 + x^2/nu)^(-(nu + 1)/2)",
                    "y = e^(-x^2/2)/sqrt(2pi)",
                ],
            )
            .sliders(&[("nu", 0.5, 30.0)])
            .window(-5.0, 5.0, -0.05, 0.5),
            entry(
                "Central limit theorem",
                "The sum of N random numbers from [0, 1] has the Irwin–Hall density. Already at N = 3 it looks almost exactly like the bell curve.",
                &[
                    "N = 3",
                    "y = sum(k, 0, N, (-1)^k binom(N, k) (x - k)^(N - 1) sign(x - k))/(2(N - 1)!)",
                    "y = e^(-(x - N/2)^2/(N/6))/sqrt(2pi N/12)",
                ],
            )
            .sliders(&[("N", 1.0, 12.0)])
            .window(-1.0, 13.0, -0.1, 1.1),
            entry(
                "Two dice",
                "Of the 36 ways two dice can land, 6 make 7, so it is the most likely total.",
                &["(n, (6 - abs(n - 7))/36)"],
            )
            .t(2.0, 12.0)
            .window(0.0, 14.0, -0.02, 0.2),
            entry(
                "Benford's law",
                "In many real data sets the first digit is 1 about 30% of the time and 9 under 5%: log10(1 + 1/n).",
                &["(n, log10(1 + 1/n))"],
            )
            .t(1.0, 9.0)
            .window(0.0, 10.0, -0.02, 0.35),
            entry(
                "Birthday problem",
                "With 23 people in a room it is more likely than not that two share a birthday.",
                &["(n, 1 - prod(k, 0, n - 1, (365 - k)/365))", "y = 0.5"],
            )
            .t(1.0, 60.0)
            .window(0.0, 61.0, -0.05, 1.05),
        ],
    ),
    folder(
        "Discrete Math & Logic",
        "Counting, combinations and the logic of sets.",
        &[
            entry(
                "Pascal's triangle mod 2",
                "Colour the odd numbers in Pascal's triangle and Sierpiński's triangle appears. Odd cells are where x and y share no binary 1s.",
                &["prod(k, 0, 5, 1 - mod(floor(x/2^k), 2) mod(floor(y/2^k), 2)) > 0.5"],
            )
            .window(0.0, 32.0, 0.0, 32.0),
            entry(
                "Binomial coefficients",
                "C(10, k), the number of ways to choose k items from 10: the 11th row of Pascal's triangle.",
                &["(n, binom(10, n))"],
            )
            .t(0.0, 10.0)
            .window(-1.0, 11.0, -10.0, 270.0),
            entry(
                "Factorial vs exponential",
                "n! eventually beats 2ⁿ, which in turn beats n².",
                &["(n, n!)", "(n, 2^n)", "(n, n^2)"],
            )
            .t(0.0, 10.0)
            .window(-0.5, 10.5, -50.0, 1200.0),
            entry(
                "Stirling's approximation",
                "n! ≈ √(2πn)(n/e)ⁿ. The error shrinks to nothing as a fraction.",
                &["(n, n!)", "y = sqrt(2pi x) (x/e)^x"],
            )
            .t(0.0, 7.0)
            .window(-0.5, 7.5, -100.0, 5500.0),
            entry(
                "Catalan numbers",
                "1, 1, 2, 5, 14, 42, …: the number of ways to bracket a product, triangulate a polygon, or draw mountain ranges.",
                &["(n, binom(2n, n)/(n + 1))"],
            )
            .t(0.0, 10.0)
            .window(-1.0, 11.0, -500.0, 17_500.0),
            entry(
                "Intersection (AND)",
                "Points in both circles: min(A, B) > 0 means both A > 0 and B > 0.",
                &["min(4 - (x + 1)^2 - y^2, 4 - (x - 1)^2 - y^2) > 0", "(x + 1)^2 + y^2 = 4", "(x - 1)^2 + y^2 = 4"],
            )
            .window(-5.0, 5.0, -3.0, 3.0),
            entry(
                "Union (OR)",
                "Points in either circle: max(A, B) > 0.",
                &["max(4 - (x + 1)^2 - y^2, 4 - (x - 1)^2 - y^2) > 0", "(x + 1)^2 + y^2 = 4", "(x - 1)^2 + y^2 = 4"],
            )
            .window(-5.0, 5.0, -3.0, 3.0),
            entry(
                "Difference (A NOT B)",
                "In the left circle but outside the right one.",
                &["min(4 - (x + 1)^2 - y^2, (x - 1)^2 + y^2 - 4) > 0", "(x + 1)^2 + y^2 = 4", "(x - 1)^2 + y^2 = 4"],
            )
            .window(-5.0, 5.0, -3.0, 3.0),
            entry(
                "Exclusive or (XOR)",
                "In exactly one circle: the product of the two tests is negative.",
                &["(4 - (x + 1)^2 - y^2)(4 - (x - 1)^2 - y^2) < 0", "(x + 1)^2 + y^2 = 4", "(x - 1)^2 + y^2 = 4"],
            )
            .window(-5.0, 5.0, -3.0, 3.0),
        ],
    ),
    folder(
        "Fractals & Chaos",
        "Infinite detail and extreme sensitivity.",
        &[
            entry(
                "Sierpiński triangle",
                "Remove the middle of every triangle, forever. Its dimension is log 3 / log 2 ≈ 1.585.",
                &["prod(k, 1, 7, 1 - mod(floor(x 2^k), 2) mod(floor(y 2^k), 2)) > 0.5"],
            )
            .window(0.0, 1.0, 0.0, 1.0),
            entry(
                "Sierpiński carpet",
                "Cut a square into nine, remove the middle one, and repeat on the other eight.",
                &[
                    "prod(k, 1, 4, 1 - (1 - abs(sign(mod(floor(x 3^k), 3) - 1))) (1 - abs(sign(mod(floor(y 3^k), 3) - 1)))) > 0.5",
                ],
            )
            .window(0.0, 1.0, 0.0, 1.0),
            entry(
                "Cantor dust",
                "Keep only the outer thirds in both directions, again and again: infinitely many points with zero total area.",
                &[
                    "prod(k, 1, 4, abs(sign(mod(floor(x 3^k), 3) - 1)) abs(sign(mod(floor(y 3^k), 3) - 1))) > 0.5",
                ],
            )
            .window(0.0, 1.0, 0.0, 1.0),
            entry(
                "Weierstrass function",
                "Continuous everywhere but smooth nowhere: zoom in and it stays just as jagged.",
                &["a = 0.5", "b = 3", "N = 8", "y = sum(k, 0, N, a^k cos(b^k pi x))"],
            )
            .sliders(&[("a", 0.1, 0.9), ("b", 1.0, 9.0), ("N", 0.0, 12.0)])
            .window(-2.0, 2.0, -2.5, 2.5),
            entry(
                "Blancmange curve",
                "Takagi's curve: tent shapes of ever smaller size stacked up. Every piece looks like the whole.",
                &["N = 8", "y = sum(k, 0, N, abs(2^k x - round(2^k x))/2^k)"],
            )
            .sliders(&[("N", 0.0, 14.0)])
            .window(-0.2, 1.2, -0.2, 1.0),
            entry(
                "Riemann's function",
                "Σ sin(k²x)/k², which Riemann suggested as a continuous function with almost no derivatives.",
                &["N = 30", "y = sum(k, 1, N, sin(k^2 x)/k^2)"],
            )
            .sliders(&[("N", 1.0, 100.0)])
            .window(-4.0, 4.0, -1.5, 1.5),
            entry(
                "Chaos in the logistic map",
                "The logistic map at r = 4 is sin²(2ⁿa). Two starts differing by 0.001 soon go completely different ways: the butterfly effect.",
                &["a = 0.5", "(n, sin(2^n a)^2)", "(n, sin(2^n (a + 0.001))^2)"],
            )
            .sliders(&[("a", 0.0, PI)])
            .t(0.0, 30.0)
            .window(-1.0, 31.0, -0.2, 1.2),
            entry(
                "Doubling map",
                "Double and keep the fractional part. The orbit jumps around, then lands on 0 after about 24 steps: the computer has used up the binary digits of a.",
                &["a = 0.3", "(n, mod(2^n a, 1))"],
            )
            .sliders(&[("a", 0.0, 1.0)])
            .t(0.0, 30.0)
            .window(-1.0, 31.0, -0.2, 1.2),
            entry(
                "Sunflower",
                "Seed n at radius √n, turned by the golden angle g = π(3 − √5). Change g even slightly and the spirals fall apart.",
                &["g = pi (3 - sqrt(5))", "(sqrt(n) cos(n g), sqrt(n) sin(n g))"],
            )
            .sliders(&[("g", 2.0, 3.0)])
            .t(1.0, 1000.0)
            .window(-40.0, 40.0, -33.0, 33.0),
        ],
    ),
    folder(
        "3D Shapes",
        "Surfaces in space: z = f(x, y) and equations in x, y and z.",
        &[
            entry(
                "Sphere",
                "x² + y² + z² = 9: every point 3 from the centre.",
                &["x^2 + y^2 + z^2 = 9"],
            )
            .cube(4.0),
            entry(
                "Ellipsoid",
                "A sphere stretched by different amounts along each axis.",
                &["x^2/9 + y^2/4 + z^2 = 1"],
            )
            .cube(4.0),
            entry(
                "Double cone",
                "x² + y² = z²: two cones meeting at a point.",
                &["x^2 + y^2 = z^2"],
            )
            .cube(4.0),
            entry(
                "Cylinder",
                "A circle swept straight up, drawn with parameters u (around) and v (up).",
                &["(2cos(u), 2sin(u), v - pi)"],
            )
            .cube(4.0),
            entry(
                "Hyperboloid of one sheet",
                "Curved, yet made entirely of straight lines. Cooling towers are built this way.",
                &["x^2 + y^2 - z^2 = 1"],
            )
            .cube(3.0),
            entry(
                "Hyperboloid of two sheets",
                "z² − x² − y² = 1: two separate bowls.",
                &["z^2 - x^2 - y^2 = 1"],
            )
            .cube(3.0),
            entry(
                "Gyroid",
                "A triply periodic minimal surface found in butterfly wings and 3D-printed lattices.",
                &["sin(x) cos(y) + sin(y) cos(z) + sin(z) cos(x) = 0"],
            )
            .cube(4.0),
            entry(
                "Heart surface",
                "A sextic surface shaped like a heart.",
                &["(x^2 + 9y^2/4 + z^2 - 1)^3 = x^2 z^3 + 9y^2 z^3/80"],
            )
            .cube(1.5),
            entry(
                "Gravity well",
                "The potential −1/r of a mass as a surface. Marbles rolling on it would orbit.",
                &["z = -1/sqrt(x^2 + y^2 + 0.2)"],
            )
            .cube(3.0),
            entry(
                "Surface of revolution",
                "Spinning the curve 2 + sin(u) around the x axis makes a vase.",
                &["(u, (2 + sin(u)) cos(v), (2 + sin(u)) sin(v))"],
            )
            .t(-4.0, 4.0)
            .v(0.0, TAU)
            .cube(5.0),
            entry(
                "Orbital sphere",
                "A helix winding up the outside of a sphere of radius a.",
                &["a = 4", "x^2 + y^2 + z^2 = a^2", "(a cos(t), a sin(t), t/2)"],
            )
            .sliders(&[("a", 1.0, 6.0)])
            .t(-8.0, 8.0)
            .cube(6.0),
            entry(
                "Saddle study",
                "A saddle, the flat plane z = 0 and a hyperboloid, showing how the shapes cross.",
                &["z = 0.25(x^2 - y^2)", "z = 0", "x^2 + y^2 - z^2 = 9"],
            )
            .cube(6.0),
            entry(
                "Torus field",
                "The torus as one implicit equation, wound with a 3D curve.",
                &[
                    "a = 3",
                    "b = 1",
                    "(x^2 + y^2 + z^2 + a^2 - b^2)^2 = 4a^2 (x^2 + y^2)",
                    "((a + b cos(t)) cos(2t), (a + b cos(t)) sin(2t), b sin(t))",
                ],
            )
            .sliders(&[("a", 1.5, 5.0), ("b", 0.3, 1.5)])
            .t(0.0, 20.0 * PI)
            .cube(5.0),
            entry(
                "Wave surfaces",
                "A still egg-crate surface and a travelling wave passing through it.",
                &["z = 2sin(x) cos(y)", "z = sin(2x + time) + cos(2y - time)"],
            )
            .cube(6.0)
            .animate(),
        ],
    ),
    folder(
        "Topology & Surfaces",
        "Parametric surfaces, knots and one-sided shapes.",
        &[
            entry(
                "Torus",
                "A circle of radius q swept around a circle of radius R: the surface of a doughnut.",
                &["R = 3", "q = 1", "((R + q cos(v)) cos(u), (R + q cos(v)) sin(u), q sin(v))"],
            )
            .sliders(&[("R", 0.5, 5.0), ("q", 0.2, 3.0)])
            .cube(5.0),
            entry(
                "Möbius strip",
                "A strip with a half twist has only one side and one edge. An ant could walk over both 'sides' without crossing an edge.",
                &["((1 + (v/pi - 1)/2 cos(u/2)) cos(u), (1 + (v/pi - 1)/2 cos(u/2)) sin(u), (v/pi - 1)/2 sin(u/2))"],
            )
            .cube(2.0),
            entry(
                "Klein bottle",
                "A one-sided closed surface. It cannot sit in 3D space without passing through itself; this is the figure-8 version.",
                &[
                    "a = 2",
                    "((a + cos(u/2) sin(v) - sin(u/2) sin(2v)) cos(u), (a + cos(u/2) sin(v) - sin(u/2) sin(2v)) sin(u), sin(u/2) sin(v) + cos(u/2) sin(2v))",
                ],
            )
            .sliders(&[("a", 1.0, 4.0)])
            .cube(4.0),
            entry(
                "Roman surface",
                "Steiner's Roman surface: a model of the projective plane that crosses itself along three lines.",
                &["(2sin(2u) cos(v)^2, 2sin(u) sin(2v), 2cos(u) sin(2v))"],
            )
            .t(0.0, PI)
            .v(-PI / 2.0, PI / 2.0)
            .cube(2.5),
            entry(
                "Helicoid",
                "A spiral staircase. It is a minimal surface, the shape of a soap film on a helix.",
                &["(v cos(u), v sin(u), (u - 2pi)/2)"],
            )
            .t(0.0, 4.0 * PI)
            .v(-2.0, 2.0)
            .cube(3.0),
            entry(
                "Catenoid",
                "The soap film stretched between two rings. Bending it turns it into the helicoid.",
                &["(cosh(v) cos(u), cosh(v) sin(u), v)"],
            )
            .v(-1.5, 1.5)
            .cube(3.0),
            entry(
                "Enneper surface",
                "A minimal surface that folds through itself.",
                &["(u - u^3/3 + u v^2, v - v^3/3 + v u^2, u^2 - v^2)"],
            )
            .t(-2.0, 2.0)
            .v(-2.0, 2.0)
            .cube(5.0),
            entry(
                "Dini's surface",
                "A twisted pseudosphere with constant negative curvature.",
                &["(cos(u) sin(v), sin(u) sin(v), cos(v) + ln(tan(v/2)) + 0.2u)"],
            )
            .t(0.0, 4.0 * PI)
            .v(0.1, 2.0)
            .cube(2.0),
            entry(
                "Seashell",
                "Growing a curve while turning it makes a shell, the way molluscs build theirs.",
                &["(1.2^v (sin(u)^2 sin(v)), 1.2^v (sin(u)^2 cos(v)), 1.2^v sin(u) cos(u))"],
            )
            .t(0.0, PI)
            .v(-PI / 4.0, 5.0 * PI / 2.0)
            .cube(3.0),
            entry(
                "Parametric sphere",
                "Longitude u and latitude v cover the sphere; the grid lines are meridians and parallels.",
                &["(3cos(u) sin(v), 3sin(u) sin(v), 3cos(v))"],
            )
            .v(0.0, PI)
            .cube(4.0),
            entry(
                "Trefoil knot",
                "The simplest true knot: it cannot be untangled into a circle without cutting.",
                &["(sin(t) + 2sin(2t), cos(t) - 2cos(2t), -sin(3t))"],
            )
            .cube(4.0),
            entry(
                "Torus knot",
                "A curve winding p times around a torus one way and q times the other. Whole numbers p and q with no common factor make a knot.",
                &["p = 2", "q = 3", "((2 + cos(q t)) cos(p t), (2 + cos(q t)) sin(p t), sin(q t))"],
            )
            .sliders(&[("p", 1.0, 9.0), ("q", 1.0, 9.0)])
            .cube(4.0),
            entry(
                "Linked rings",
                "Two circles each passing through the other: the Hopf link, the simplest link.",
                &["(cos(t), sin(t), 0)", "(1 + cos(t), 0, sin(t))"],
            )
            .cube(2.0),
        ],
    ),
    folder(
        "Physics",
        "Motion, waves, fields and orbits.",
        &[
            entry(
                "Projectile motion",
                "Thrown at speed v0 and angle ang, the path is a parabola. 45° (0.785) goes furthest.",
                &["v0 = 12", "ang = 0.8", "(v0 cos(ang) t, v0 sin(ang) t - 4.9t^2)"],
            )
            .sliders(&[("v0", 1.0, 20.0), ("ang", 0.0, 1.57)])
            .t(0.0, 3.0)
            .window(-1.0, 16.0, -1.0, 8.0),
            entry(
                "Range vs launch angle",
                "The landing distance v0²·sin(2θ)/g peaks at 45°. Angles that add to 90° land at the same spot.",
                &["v0 = 12", "y = v0^2 sin(2x)/9.8"],
            )
            .sliders(&[("v0", 1.0, 20.0)])
            .window(0.0, 1.6, -1.0, 16.0),
            entry(
                "Simple harmonic motion",
                "A mass on a spring: x = A·cos(wt). The dot moves along the graph as time passes.",
                &["A = 2", "w = 2", "y = A cos(w x)", "(mod(time, 10), A cos(w mod(time, 10)))"],
            )
            .sliders(&[("A", 0.2, 4.0), ("w", 0.2, 5.0)])
            .window(-0.5, 10.0, -3.0, 3.0)
            .animate(),
            entry(
                "Damped oscillation",
                "Friction shrinks the swings inside an exponential envelope ±e^(−cx).",
                &["c = 0.3", "w = 4", "y = e^(-c x) cos(w x)", "y = e^(-c x)", "y = -e^(-c x)"],
            )
            .sliders(&[("c", 0.0, 2.0), ("w", 0.5, 10.0)])
            .window(-0.5, 12.0, -1.5, 1.5),
            entry(
                "Resonance",
                "Push a swing at its natural frequency (x = 1) and the amplitude soars. Less damping c means a taller, sharper peak.",
                &["c = 0.1", "y = 1/sqrt((1 - x^2)^2 + (2c x)^2)"],
            )
            .sliders(&[("c", 0.02, 1.0)])
            .window(0.0, 3.0, -0.5, 6.0),
            entry(
                "Standing wave",
                "A string fixed at the nodes vibrates up and down without travelling, like a guitar string.",
                &["y = sin(2x) cos(2time)", "y = sin(2x)", "y = -sin(2x)"],
            )
            .window(-5.0, 5.0, -2.0, 2.0)
            .animate(),
            entry(
                "Travelling waves add up",
                "Two waves moving in opposite directions add up to a standing wave.",
                &["y = sin(x - time)/2", "y = sin(x + time)/2", "y = sin(x - time)/2 + sin(x + time)/2"],
            )
            .window(-7.0, 7.0, -1.5, 1.5)
            .animate(),
            entry(
                "Two-source interference",
                "Ripples from two sources add up in some places and cancel in others: the double-slit pattern.",
                &["sin(4hypot(x - 2, y) - 3time) + sin(4hypot(x + 2, y) - 3time) > 0.8"],
            )
            .window(-8.0, 8.0, -5.0, 5.0)
            .animate(),
            entry(
                "Kepler orbit",
                "Planets move on ellipses with the Sun at one focus. Eccentricity ecc sets how stretched the orbit is.",
                &["p = 4", "ecc = 0.6", "r = p/(1 + ecc cos(θ))", "(0, 0)"],
            )
            .sliders(&[("p", 1.0, 8.0), ("ecc", 0.0, 0.95)])
            .window(-14.0, 6.0, -6.0, 6.0),
            entry(
                "Electric dipole",
                "The field of a positive and a negative charge. Field lines leave one and end on the other.",
                &[
                    "((x - 1)/hypot(x - 1, y)^3 - (x + 1)/hypot(x + 1, y)^3, y/hypot(x - 1, y)^3 - y/hypot(x + 1, y)^3)",
                    "(1, 0)",
                    "(-1, 0)",
                ],
            )
            .window(-4.0, 4.0, -2.5, 2.5),
            entry(
                "Blackbody spectrum",
                "Planck's law: hotter objects glow brighter and peak at higher frequencies, from red-hot to white-hot.",
                &["T = 1", "y = x^3/(e^(x/T) - 1)"],
            )
            .sliders(&[("T", 0.3, 2.5)])
            .window(0.0, 15.0, -0.2, 5.0),
            entry(
                "Thin lens",
                "The image distance for an object at distance x in front of a lens with focal length f. At x = f the image is at infinity.",
                &["f = 2", "y = f x/(x - f)", "x = f", "y = f"],
            )
            .sliders(&[("f", 0.5, 5.0)])
            .window(0.0, 12.0, -10.0, 15.0),
            entry(
                "Atomic orbital (d_z²)",
                "The shape of a d orbital: the radius follows |3cos²v − 1|, two lobes with a ring around the middle.",
                &[
                    "(abs(3cos(v)^2 - 1) cos(u) sin(v), abs(3cos(v)^2 - 1) sin(u) sin(v), abs(3cos(v)^2 - 1) cos(v))",
                ],
            )
            .v(0.0, PI)
            .cube(2.0),
        ],
    ),
    folder(
        "Signals & Engineering",
        "Waves, modulation, filters and sampling.",
        &[
            entry(
                "Amplitude modulation",
                "AM radio: a slow message changes the height of a fast carrier wave.",
                &["m = 0.5", "y = (1 + m cos(x)) cos(12x)", "y = 1 + m cos(x)", "y = -1 - m cos(x)"],
            )
            .sliders(&[("m", 0.0, 1.0)])
            .window(-7.0, 7.0, -2.2, 2.2),
            entry(
                "Frequency modulation",
                "FM radio and synthesisers: the message speeds the carrier up and slows it down. b is the modulation index.",
                &["b = 3", "y = cos(8x + b sin(x))"],
            )
            .sliders(&[("b", 0.0, 10.0)])
            .window(-7.0, 7.0, -1.5, 1.5),
            entry(
                "Beats",
                "Two close notes, 10 and 11, pulse at their difference frequency. Musicians use this to tune.",
                &["y = sin(10x) + sin(11x)", "y = 2cos(x/2)", "y = -2cos(x/2)"],
            )
            .window(-7.0, 7.0, -2.5, 2.5),
            entry(
                "Sinc pulse",
                "sin(πx)/(πx), the ideal low-pass filter and the shape behind sampling theory.",
                &["y = sin(pi x)/(pi x)"],
            )
            .window(-8.0, 8.0, -0.5, 1.2),
            entry(
                "Wave packet",
                "A wave inside a Gaussian envelope, a pulse with a definite place and pitch.",
                &["y = e^(-x^2/4) cos(6x)", "y = e^(-x^2/4)", "y = -e^(-x^2/4)"],
            )
            .window(-7.0, 7.0, -1.5, 1.5),
            entry(
                "Sampling and aliasing",
                "Samples every 0.5. Above frequency π (the Nyquist limit), the dots trace a slower false wave.",
                &["f = 1", "y = sin(f x)", "(n/2, sin(f n/2))"],
            )
            .sliders(&[("f", 0.0, 12.0)])
            .t(-20.0, 20.0)
            .window(-10.0, 10.0, -1.5, 1.5),
            entry(
                "RC charging",
                "A capacitor charging through a resistor reaches 63% after one time constant rc.",
                &["rc = 1", "y = (1 - e^(-x/rc)) (1 + sign(x))/2", "y = 1 - e^(-1)"],
            )
            .sliders(&[("rc", 0.1, 4.0)])
            .window(-1.0, 8.0, -0.2, 1.3),
            entry(
                "Bode plot (low-pass)",
                "Gain in decibels against log frequency: flat, then falling 20 dB per decade after the corner.",
                &["y = -10log10(1 + 10^(2x))", "y = 0", "y = -20x"],
            )
            .window(-2.0, 2.0, -45.0, 5.0),
            entry(
                "Pulse width modulation",
                "Switching on for a fraction d of each cycle sets the average power. LED dimmers and motor drivers work this way.",
                &["d = 0.3", "y = (1 - sign(mod(x, 1) - d))/2", "y = d"],
            )
            .sliders(&[("d", 0.0, 1.0)])
            .window(-0.5, 4.5, -0.3, 1.3),
        ],
    ),
    folder(
        "Economics & Biology",
        "Markets, money and living populations.",
        &[
            entry(
                "Supply and demand",
                "The price settles where supply meets demand. Raise demand d and the price and quantity both go up.",
                &["d = 8", "y = 1 + 0.5x", "y = d - x", "((d - 1)/1.5, 1 + (d - 1)/3)"],
            )
            .sliders(&[("d", 2.0, 12.0)])
            .window(-1.0, 10.0, -1.0, 9.0),
            entry(
                "Compound interest",
                "Compounding m times a year approaches continuous growth P·e^(rate·x).",
                &["P = 100", "rate = 0.08", "m = 1", "y = P (1 + rate/m)^(m x)", "y = P e^(rate x)"],
            )
            .sliders(&[("P", 10.0, 1000.0), ("rate", 0.0, 0.2), ("m", 1.0, 365.0)])
            .window(0.0, 30.0, 0.0, 1200.0),
            entry(
                "Logistic population",
                "Growth slows as resources run out, levelling off at the carrying capacity K.",
                &["K = 10", "P = 0.5", "g = 1", "y = K/(1 + (K/P - 1) e^(-g x))", "y = K"],
            )
            .sliders(&[("K", 1.0, 20.0), ("P", 0.1, 10.0), ("g", 0.1, 3.0)])
            .window(-1.0, 12.0, -1.0, 12.0),
            entry(
                "Cost, revenue and profit",
                "Profit is revenue minus cost. The business breaks even where the lines cross, at 10 units.",
                &["y = 50 + 2x", "y = 7x", "y = 5x - 50"],
            )
            .window(-1.0, 20.0, -60.0, 150.0),
            entry(
                "Michaelis–Menten",
                "An enzyme's reaction rate levels off at Vm. Km is the concentration that gives half speed.",
                &["Vm = 5", "Km = 2", "y = Vm x/(Km + x)", "y = Vm", "x = Km"],
            )
            .sliders(&[("Vm", 1.0, 10.0), ("Km", 0.2, 10.0)])
            .window(-1.0, 20.0, -1.0, 7.0),
            entry(
                "Kleiber's law",
                "An animal's metabolism grows as mass^0.75, so big animals live more efficiently per kilogram.",
                &["y = x^0.75", "y = x"],
            )
            .window(0.0, 10.0, 0.0, 10.0),
            entry(
                "Lorenz curve and Gini",
                "Share of income held by the poorest fraction x. The shaded gap below the line of equality measures inequality.",
                &["p = 2", "y = x^p", "y = x", "min(min(x - y, y - x^p), min(x, 1 - x)) > 0"],
            )
            .sliders(&[("p", 1.0, 6.0)])
            .window(-0.1, 1.1, -0.1, 1.1),
            entry(
                "Rabbits and foxes",
                "Predator and prey populations cycling: more rabbits feed more foxes, which then eat the rabbits down.",
                &["(x - 0.5x y, 0.5x y - y)"],
            )
            .window(-0.5, 6.0, -0.5, 6.0),
        ],
    ),
    folder(
        "Famous Curves",
        "Curves with names and stories.",
        &[
            entry(
                "Witch of Agnesi",
                "Studied by Maria Agnesi in 1748. The name comes from a mistranslation of the Italian for 'curve'.",
                &["a = 1", "y = 8a^3/(x^2 + 4a^2)"],
            )
            .sliders(&[("a", 0.2, 3.0)])
            .window(-8.0, 8.0, -1.0, 3.0),
            entry(
                "Folium of Descartes",
                "x³ + y³ = 3axy, a leaf with a loop that Descartes challenged Fermat to find tangents to.",
                &["a = 2", "x^3 + y^3 = 3a x y"],
            )
            .sliders(&[("a", 0.5, 4.0)]),
            entry(
                "Astroid",
                "A four-cusped star traced by a circle rolling inside another four times its size.",
                &["(4cos(t)^3, 4sin(t)^3)"],
            ),
            entry(
                "Deltoid",
                "Three cusps: a circle rolling inside one three times its size.",
                &["(2cos(t) + cos(2t), 2sin(t) - sin(2t))"],
            )
            .window(-4.0, 4.0, -2.5, 2.5),
            entry(
                "Nephroid",
                "Kidney-shaped. The bright curve of light in a coffee cup is half a nephroid.",
                &["(3cos(t) - cos(3t), 3sin(t) - sin(3t))"],
            )
            .window(-6.0, 6.0, -4.5, 4.5),
            entry(
                "Cissoid of Diocles",
                "Used in ancient Greece to double the cube.",
                &["a = 1", "y^2 (2a - x) = x^3"],
            )
            .sliders(&[("a", 0.2, 3.0)])
            .window(-2.0, 4.0, -4.0, 4.0),
            entry(
                "Catenary",
                "The shape of a hanging chain, y = a·cosh(x/a). Turned upside down it makes the strongest arch.",
                &["a = 2", "y = a cosh(x/a)"],
            )
            .sliders(&[("a", 0.3, 5.0)])
            .window(-8.0, 8.0, -1.0, 9.0),
            entry(
                "Tractrix",
                "Drag a weight on a rope along a straight path and it follows this curve.",
                &["(t - tanh(t), 1/cosh(t))"],
            )
            .t(-5.0, 5.0)
            .window(-5.0, 5.0, -0.5, 1.5),
            entry(
                "Lemniscate of Bernoulli",
                "(x² + y²)² = 2a²(x² − y²), the figure eight behind the infinity symbol ∞.",
                &["a = 3", "(x^2 + y^2)^2 = 2a^2 (x^2 - y^2)"],
            )
            .sliders(&[("a", 0.5, 5.0)]),
            entry(
                "Devil's curve",
                "y²(y² − 4) = x²(x² − 5), named for its resemblance to an old game, 'devil on two sticks'.",
                &["y^2 (y^2 - 4) = x^2 (x^2 - 5)"],
            )
            .window(-4.0, 4.0, -3.0, 3.0),
            entry(
                "Conchoid of Nicomedes",
                "Used by the Greeks to trisect angles. Changing a and b gives loops, cusps or smooth humps.",
                &["a = 2", "b = 1", "(x - b)^2 (x^2 + y^2) = a^2 x^2"],
            )
            .sliders(&[("a", 0.2, 4.0), ("b", 0.2, 3.0)])
            .window(-4.0, 6.0, -4.0, 4.0),
            entry(
                "Euler spiral",
                "Curvature grows steadily along the curve. Railways and roads use it so that turns feel smooth.",
                &[
                    "(sum(k, 1, 200, cos((k t/200)^2)) t/200, sum(k, 1, 200, sin((k t/200)^2)) t/200)",
                ],
            )
            .t(-8.0, 8.0)
            .window(-1.6, 1.6, -1.0, 1.0),
            entry(
                "Bean curve",
                "(x² + y²)² = x³ + y³.",
                &["(x^2 + y^2)^2 = x^3 + y^3"],
            )
            .window(-0.6, 1.6, -0.7, 1.4),
            entry(
                "Trisectrix of Maclaurin",
                "A cubic curve that trisects any angle.",
                &["a = 1", "2x (x^2 + y^2) = a (3x^2 - y^2)"],
            )
            .sliders(&[("a", 0.2, 3.0)])
            .window(-3.0, 3.0, -2.0, 2.0),
            entry(
                "Kampyle of Eudoxus",
                "x⁴ = a²(x² + y²), studied in ancient Greece for doubling the cube.",
                &["a = 1", "x^4 = a^2 (x^2 + y^2)"],
            )
            .sliders(&[("a", 0.2, 3.0)]),
            entry(
                "Bicorn",
                "The cocked-hat curve.",
                &["a = 2", "y^2 (a^2 - x^2) = (x^2 + 2a y - a^2)^2"],
            )
            .sliders(&[("a", 0.5, 4.0)])
            .window(-3.0, 3.0, -1.0, 3.0),
            entry(
                "Superellipse",
                "|x/a|ᵐ + |y/b|ᵐ = 1. m = 2 is an ellipse; larger m gives a squircle, as in app icons.",
                &["a = 4", "b = 3", "m = 4", "abs(x/a)^m + abs(y/b)^m = 1"],
            )
            .sliders(&[("a", 0.5, 8.0), ("b", 0.5, 5.0), ("m", 0.3, 10.0)]),
            entry(
                "Piriform",
                "A teardrop-shaped quartic curve.",
                &["(2(1 + sin(t)), cos(t) (1 + sin(t)))"],
            )
            .window(-1.0, 5.0, -2.0, 2.0),
        ],
    ),
    folder(
        "Art & Motion",
        "Moving, playful pictures: good material for visuals.",
        &[
            entry(
                "Breathing rose",
                "A five-petal flower whose petals swell and turn with time.",
                &["r = 3 + 2cos(5θ + time)"],
            )
            .animate(),
            entry(
                "Spinning spirograph",
                "The spirograph's inner wheel keeps turning, so the figure never repeats exactly.",
                &["((5 - 3) cos(t) + 4cos(2/3 t + time), (5 - 3) sin(t) - 4sin(2/3 t + time))"],
            )
            .t(0.0, 6.0 * PI)
            .window(-9.0, 9.0, -6.0, 6.0)
            .animate(),
            entry(
                "Moiré rings",
                "Two sets of rings overlap into ghostly fringe patterns. Slide one set across with a.",
                &["a = 1", "sin(8hypot(x, y)) sin(8hypot(x - a, y)) > 0"],
            )
            .sliders(&[("a", 0.0, 4.0)])
            .window(-6.0, 6.0, -4.0, 4.0),
            entry(
                "Plasma",
                "Three moving waves added and thresholded make a lava-lamp blob.",
                &["sin(x + time) + sin(y/2 + time) + sin(hypot(x, y) - 2time) > 0.5"],
            )
            .window(-10.0, 10.0, -6.0, 6.0)
            .animate(),
            entry(
                "Shifting sunflower",
                "The golden-angle sunflower with an angle that wobbles slightly, sweeping through spiral families.",
                &["(sqrt(n) cos(n (2.39996 + 0.002sin(time/3))), sqrt(n) sin(n (2.39996 + 0.002sin(time/3))))"],
            )
            .t(1.0, 800.0)
            .window(-36.0, 36.0, -30.0, 30.0)
            .animate(),
            entry(
                "Kaleidoscope",
                "Rings with 6- and 12-fold symmetry turning against each other.",
                &["min(4 + 2sin(6atan2(y, x) + time) - hypot(x, y), hypot(x, y) - 2 - sin(12atan2(y, x) - time)) > 0"],
            )
            .window(-8.0, 8.0, -5.0, 5.0)
            .animate(),
            entry(
                "Lissajous dance",
                "A Lissajous figure whose phase drifts, so it seems to turn in 3D.",
                &["(4sin(3t + time), 4sin(2t))"],
            )
            .animate(),
            entry(
                "Lissajous knot",
                "Three perpendicular vibrations at frequencies 3, 2 and 5 tie a knot in space.",
                &["(3cos(3t + 0.7), 3cos(2t + 0.2), 3cos(5t))"],
            )
            .cube(4.0),
            entry(
                "Breathing torus",
                "A torus whose tube swells and shrinks with time.",
                &[
                    "((3 + (1 + 0.4sin(time)) cos(v)) cos(u), (3 + (1 + 0.4sin(time)) cos(v)) sin(u), (1 + 0.4sin(time)) sin(v))",
                ],
            )
            .cube(5.0)
            .animate(),
            entry(
                "Ripple pond",
                "Circular waves spreading outward forever.",
                &["z = cos(2hypot(x, y) - 2time)/(1 + hypot(x, y)/3)"],
            )
            .cube(8.0)
            .animate(),
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphing::{Plot, PlotKind, Scope, View, classify};
    use std::collections::HashSet;

    fn check_folder(folder: &LibraryFolder, path: &str) {
        let mut titles = HashSet::new();
        assert!(
            folder.entry_count() > 0,
            "{path}{} has no examples",
            folder.name
        );
        for entry in folder.entries {
            let at = format!("{path}{} › {}", folder.name, entry.title);
            assert!(titles.insert(entry.title), "duplicate title {at}");
            assert!(!entry.lines.is_empty() && !entry.blurb.is_empty(), "{at}");
            let mut scope = Scope::default();
            for line in entry.lines {
                if let Ok(PlotKind::Slider { name, value }) = classify(line) {
                    if let Some([min, max]) = entry.slider_range(&name) {
                        assert!(
                            (min..=max).contains(&value),
                            "{at}: slider {name} = {value} outside {min}..{max}"
                        );
                    }
                    scope.names.push(name);
                    scope.values.push(value);
                }
            }
            for (name, ..) in entry.sliders {
                assert!(
                    scope.names.iter().any(|slider| slider == name),
                    "{at}: range for missing slider {name}"
                );
            }
            let view = entry.window.map_or(
                View {
                    x: [-10.0, 10.0],
                    y: [-6.0, 6.0],
                },
                |[x0, x1, y0, y1]| View {
                    x: [x0, x1],
                    y: [y0, y1],
                },
            );
            scope.time = 1.3;
            let mut animated = false;
            for line in entry.lines {
                let kind = classify(line).unwrap_or_else(|error| panic!("{at}: {line}: {error}"));
                animated |= line.contains("time");
                if matches!(kind, PlotKind::Slider { .. }) {
                    continue;
                }
                let parameter = entry.parameter.unwrap_or(kind.default_parameter());
                let secondary = entry.secondary.unwrap_or([0.0, TAU]);
                let plot = Plot::new(kind.clone(), &scope)
                    .unwrap_or_else(|error| panic!("{at}: {line}: {error}"));
                let drawn = if kind.is_3d() {
                    let s = plot.sample_3d(&scope, view, 24, parameter, secondary);
                    s.grid
                        .iter()
                        .chain(&s.curve)
                        .chain(s.triangles.iter().flatten())
                        .any(|p| p.iter().all(|c| c.is_finite()))
                } else {
                    let p = plot.sample(&scope, view, 600, parameter);
                    let finite = |p: &[f32; 2]| p.iter().all(|c| c.is_finite());
                    p.polylines.iter().flatten().any(finite)
                        || p.points.iter().any(finite)
                        || !p.segments.is_empty()
                        || !p.cells.is_empty()
                        || !p.arrows.is_empty()
                };
                assert!(drawn, "{at}: {line} draws nothing in its window");
            }
            assert_eq!(animated, entry.animate, "{at}: animate flag");
        }
        for sub in folder.folders {
            check_folder(sub, &format!("{path}{} › ", folder.name));
        }
    }

    #[test]
    fn every_example_parses_and_draws() {
        for folder in LIBRARY {
            check_folder(folder, "");
        }
        assert!(entry_count() >= 250, "library has {}", entry_count());
    }

    #[test]
    fn search_matches_titles_folders_and_equations() {
        assert!(search("taylor").len() >= 4);
        assert!(
            search("möbius")
                .iter()
                .any(|(_, e)| e.title == "Möbius strip")
        );
        assert!(
            search("conic hyperbola")
                .iter()
                .any(|(path, _)| path == "Conic Sections")
        );
        assert!(search("isprime").len() >= 3);
        assert!(search("no such thing anywhere").is_empty());
        assert_eq!(search("").len(), entry_count());
    }
}
