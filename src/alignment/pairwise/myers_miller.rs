// Copyright 2020 Tianyi Shi
// Licensed under the MIT license (http://opensource.org/licenses/MIT)
// This file may not be copied, modified, or distributed
// except according to those terms.

//! Alignment with affine gap penalty in linear space, by combining Gotoh's (1982) and
//! Hirschberg's (1975) ideas, which was first implemented in C (Myers & Miller 1988).
//!
//! # Time Complexity
//!
//! O(n * m) for strings of length m and n.
//!
//! # Space Complexity
//!
//! The space usage depends on the `cost_only` method of [Aligner](struct.Aligner),
//! which uses 6 scalars and 2 vectors of length (n + 1), where n is the length of the shorter sequence.
//! [See also](struct.Aligner.html#space-complexity)
//!
//! # References
//!
//! - [Eugene W. Myers and Webb Miller (1988) Optimal alignments in linear space. _Bioinformatics_ **4**: 11-17.](https://doi.org/10.1093/bioinformatics/4.1.11)
//! - [Hirschberg, D. S. (1975) A linear space algorithm for computing maximal common subsequences. _Commun. Assoc. Comput. Mach._ **18**: 341-343.](https://doi.org/10.1145/360825.360861)
//! - [Gotoh, O. (1982) An improved algorithm for matching biological sequences. _J. Molec. Biol._ **162**: 705-708.](https://doi.org/10.1016/0022-2836(82)90398-9)

use crate::alignment::pairwise::{MatchFunc, Scoring};
use crate::alignment::{Alignment, AlignmentMode, AlignmentOperation};
use crate::utils::TextSlice;
use std::cmp::max;

pub struct Aligner<F: MatchFunc> {
    scoring: Scoring<F>,
}

impl<F: MatchFunc> Aligner<F> {
    /// Create new aligner instance with given gap open and gap extend penalties
    /// and the score function.
    ///
    /// # Arguments
    ///
    /// * `gap_open` - the score for opening a gap (should be negative)
    /// * `gap_extend` - the score for extending a gap (should be negative)
    /// * `match_fn` - function that returns the score for substitutions (also see bio::scores)
    pub fn new(gap_open: i32, gap_extend: i32, match_fn: F) -> Self {
        Aligner {
            scoring: Scoring::new(gap_open, gap_extend, match_fn),
        }
    }
    pub fn global(&self, x: TextSlice, y: TextSlice) -> Alignment {
        let (m, n) = (x.len(), y.len());
        let operations =
            self.compute_recursive(x, y, m, n, self.scoring.gap_open, self.scoring.gap_open);
        let score = self.cost_only(x, y, false, self.scoring.gap_open).0[y.len()];
        return Alignment {
            score,
            xstart: 0,
            ystart: 0,
            xend: m,
            yend: n,
            xlen: m,
            ylen: n,
            operations,
            mode: AlignmentMode::Global,
        };
    }
    /// Recursively compute alignments of sub-sequences and concatenating them
    fn compute_recursive(
        &self,
        x: TextSlice,
        y: TextSlice,
        m: usize,
        n: usize,
        tb: i32,
        te: i32,
    ) -> Vec<AlignmentOperation> {
        // * m = x.len(); n = y.len()
        if n == 0 {
            return vec![AlignmentOperation::Ins; m];
        }
        if m == 0 {
            return vec![AlignmentOperation::Del; n];
        }
        if m == 1 {
            return self.nw_onerow(x[0], y, n, tb, te);
        }
        let (imid, jmid, join_by_deletion) = self.find_mid(x, y, m, n, tb, te);
        return if join_by_deletion {
            [
                self.compute_recursive(&x[..imid - 1], &y[..jmid], imid - 1, jmid, tb, 0),
                vec![AlignmentOperation::Ins; 2],
                self.compute_recursive(&x[imid + 1..], &y[jmid..], m - imid - 1, n - jmid, 0, te),
            ]
            .concat()
        } else {
            [
                self.compute_recursive(
                    &x[..imid],
                    &y[..jmid],
                    imid,
                    jmid,
                    tb,
                    self.scoring.gap_open,
                ),
                self.compute_recursive(
                    &x[imid..],
                    &y[jmid..],
                    m - imid,
                    n - jmid,
                    self.scoring.gap_open,
                    te,
                ),
            ]
            .concat()
        };
    }

    fn find_mid(
        &self,
        x: TextSlice,
        y: TextSlice,
        m: usize,
        n: usize,
        tb: i32,
        te: i32,
    ) -> (usize, usize, bool) {
        let imid = m / 2;
        let (cc_upper, dd_upper) = self.cost_only(&x[..imid], y, false, tb);
        let (cc_lower, dd_lower) = self.cost_only(&x[imid..], y, true, te);
        let mut max = i32::MIN;
        let mut jmid = 0;
        let mut join_by_deletion = false;
        for j in 0..=n {
            let c = cc_upper[j] + cc_lower[n - j];
            if c > max {
                max = c;
                jmid = j;
                join_by_deletion = false;
            }
            let d = dd_upper[j] + dd_lower[n - j] - self.scoring.gap_open; // subtract duplicating open!
            if d > max {
                max = d;
                jmid = j;
                join_by_deletion = true;
            }
        }
        (imid, jmid, join_by_deletion)
    }

    /// Cost-only (score-only) Gotoh's algorithm in linear space
    /// # Space Complexity
    /// Use six scalars and two vectors of length (N + 1), where N is the length
    /// of the shorter sequence.
    fn cost_only(&self, x: TextSlice, y: TextSlice, rev: bool, tx: i32) -> (Vec<i32>, Vec<i32>) {
        let m = x.len() + 1;
        let n = y.len() + 1;
        let mut cc: Vec<i32> = vec![0; n]; // match/mismatch
        let mut dd: Vec<i32> = vec![0; n]; // deletion
        let mut e: i32; // I(i, j-1)
        let mut c: i32; // C(i, j-1)
        let mut s: i32; // C(i-1, j-1)
        let mut t: i32;
        t = self.scoring.gap_open;
        for j in 1..n {
            t += self.scoring.gap_extend;
            cc[j] = t;
            dd[j] = i32::MIN;
        }
        t = tx; // originally self.scoring.gap_open;
        for i in 1..m {
            s = cc[0];
            t += self.scoring.gap_extend;
            c = t;
            cc[0] = c;
            // dd[0] = c;
            e = i32::MIN;
            for j in 1..n {
                e = max(e, c + self.scoring.gap_open) + self.scoring.gap_extend; // update e to I[i,j]
                dd[j] = max(dd[j], cc[j] + self.scoring.gap_open) + self.scoring.gap_extend; // cc[j] = C[i-1, j]
                c = if rev {
                    max(
                        max(dd[j], e),
                        s + self.scoring.match_fn.score(x[m - i - 1], y[n - j - 1]),
                    )
                } else {
                    max(
                        max(dd[j], e),
                        s + self.scoring.match_fn.score(x[i - 1], y[j - 1]),
                    )
                };
                s = cc[j];
                cc[j] = c;
            }
        }
        dd[0] = cc[0]; // otherwise indels at start/end will be free
        (cc, dd)
    }
    fn nw_onerow(
        &self,
        x: u8,
        y: TextSlice,
        n: usize,
        tb: i32,
        te: i32,
    ) -> Vec<AlignmentOperation> {
        let score_by_indels_only =
            max(tb, te) + self.scoring.gap_extend * (n as i32 + 1) + self.scoring.gap_open;
        let mut max = score_by_indels_only;
        let score_with_one_substitution_base =
            (n as i32 - 1) * self.scoring.gap_extend + self.scoring.gap_open; // plus substitution score and possibly one more gap_open
        let mut maxj_ = 0usize;
        for j_ in 0..n {
            // index of sequence instead of matrix; y[j] instead of j[j-1] is the jth character
            let score = score_with_one_substitution_base
                + self.scoring.match_fn.score(x, y[j_])
                + if j_ == 0 || j_ == n - 1 {
                    0
                } else {
                    self.scoring.gap_open
                };
            if score > max {
                max = score;
                maxj_ = j_;
            }
        }
        return if max == score_by_indels_only {
            let mut res = Vec::with_capacity(n + 1);
            res.push(AlignmentOperation::Ins);
            for _j in 0..n {
                res.push(AlignmentOperation::Del)
            }
            res
        } else {
            let mut res = Vec::with_capacity(n);
            for _j in 0..maxj_ {
                res.push(AlignmentOperation::Del)
            }
            if x == y[maxj_] {
                res.push(AlignmentOperation::Match);
            } else {
                res.push(AlignmentOperation::Subst);
            }
            for _j in 0..(n - maxj_ - 1) {
                res.push(AlignmentOperation::Del)
            }
            res
        };
    }
}

// copied from pariwise/mod.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::alignment::AlignmentOperation::*;
    use crate::scores::blosum62;
    use std::iter::repeat;

    #[test]
    fn test_global_affine_ins() {
        let x = b"ACGAGAACA";
        let y = b"ACGACA";
        let score = |a: u8, b: u8| if a == b { 1i32 } else { -3i32 };
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);

        println!("aln:\n{}", alignment.pretty(x, y));
        assert_eq!(
            alignment.operations,
            [Match, Match, Match, Ins, Ins, Ins, Match, Match, Match]
        );
    }

    #[test]
    fn test_global_affine_ins2() {
        let x = b"AGATAGATAGATAGGGAGTTGTGTAGATGATCCACAGT";
        let y = b"AGATAGATAGATGTAGATGATCCACAGT";
        let score = |a: u8, b: u8| if a == b { 1i32 } else { -1i32 };
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);

        println!("aln:\n{}", alignment.pretty(x, y));

        let mut correct = Vec::new();
        correct.extend(repeat(Match).take(11));
        correct.extend(repeat(Ins).take(10));
        correct.extend(repeat(Match).take(17));

        assert_eq!(alignment.operations, correct);
    }

    #[test]
    fn test_global() {
        let x = b"ACCGTGGAT";
        let y = b"AAAAACCGTTGAT";
        let score = |a: u8, b: u8| if a == b { 1i32 } else { -1i32 };
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);

        println!("\naln:\n{}", alignment.pretty(x, y));
        assert_eq!(alignment.ystart, 0);
        assert_eq!(alignment.xstart, 0);
        assert_eq!(
            alignment.operations,
            [Del, Del, Del, Del, Match, Match, Match, Match, Match, Subst, Match, Match, Match,]
        );
    }

    #[test]
    fn test_blosum62() {
        let x = b"AAAA";
        let y = b"AAAA";
        let score = &blosum62;
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);
        assert_eq!(alignment.ystart, 0);
        assert_eq!(alignment.xstart, 0);
        assert_eq!(alignment.score, 16);
        assert_eq!(alignment.operations, [Match, Match, Match, Match]);
    }

    #[test]
    fn test_issue11() {
        let y = b"TACC"; //GTGGAC";
        let x = b"AAAAACC"; //GTTGACGCAA";
        let score = |a: u8, b: u8| if a == b { 1i32 } else { -1i32 };
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);
        assert_eq!(alignment.ystart, 0);
        assert_eq!(alignment.xstart, 0);
        assert_eq!(
            alignment.operations,
            [Ins, Ins, Ins, Subst, Match, Match, Match]
        );
    }

    #[test]
    fn test_left_aligned_del() {
        let x = b"GTGCATCATGTG";
        let y = b"GTGCATCATCATGTG";
        let score = |a: u8, b: u8| if a == b { 1i32 } else { -1i32 };
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);
        println!("\naln:\n{}", alignment.pretty(x, y));

        assert_eq!(alignment.ystart, 0);
        assert_eq!(alignment.xstart, 0);
        assert_eq!(
            alignment.operations,
            [
                Match, Match, Match, Del, Del, Del, Match, Match, Match, Match, Match, Match,
                Match, Match, Match,
            ]
        );
    }

    // Test that trailing deletions are correctly handled
    // in global mode
    #[test]
    fn test_global_right_del() {
        let x = b"AACCACGTACGTGGGGGGA";
        let y = b"CCACGTACGT";

        let score = |a: u8, b: u8| if a == b { 1i32 } else { -1i32 };
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);

        println!("\naln:\n{}", alignment.pretty(x, y));

        println!("score:{}", alignment.score);
        assert_eq!(alignment.score, -9);
        assert_eq!(alignment.ystart, 0);
        assert_eq!(alignment.xstart, 0);
        assert_eq!(
            alignment.operations,
            [
                Ins, Ins, Match, Match, Match, Match, Match, Match, Match, Match, Match, Match,
                Ins, Ins, Ins, Ins, Ins, Ins, Ins,
            ]
        );
    }

    #[test]
    fn test_left_aligned_ins() {
        let x = b"GTGCATCATCATGTG";
        let y = b"GTGCATCATGTG";
        let score = |a: u8, b: u8| if a == b { 1i32 } else { -1i32 };
        let aligner = Aligner::new(-5, -1, score);
        let alignment = aligner.global(x, y);
        println!("\naln:\n{}", alignment.pretty(x, y));

        assert_eq!(alignment.ystart, 0);
        assert_eq!(alignment.xstart, 0);
        assert_eq!(
            alignment.operations,
            [
                Match, Match, Match, Ins, Ins, Ins, Match, Match, Match, Match, Match, Match,
                Match, Match, Match,
            ]
        );
    }
}
