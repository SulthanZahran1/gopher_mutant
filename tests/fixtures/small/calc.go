// Package calc is the small fixture for gopher_mutant's M1 engine.
//
// Sizing contract (wayfinder #7): 1-2 files, 20-30 mutants, 100% kill,
// <5s wall clock. Every construct below is deliberate — see calc_test.go
// for the kill conditions (boundary inputs distinguish every mutant).
package calc

// Add returns a + b.
func Add(a, b int) int {
	return a + b
}

// Mul returns a * b.
func Mul(a, b int) int {
	return a * b
}

// IsPositive reports whether x > 0. The 0 boundary is what kills the
// ROR `>`→`>=` mutant: 0 is not positive, but 0 >= 0 is true.
func IsPositive(x int) bool {
	return x > 0
}

// IsInRange reports whether v is in [lo, hi] (inclusive).
// Boundary inputs v == lo and v == hi kill the `>=`→`>` / `<=`→`<`
// mutants; out-of-range inputs kill the `&&`→`||` and term-removal mutants.
func IsInRange(v, lo, hi int) bool {
	return v >= lo && v <= hi
}

// Sum adds all values in xs. The `s := 0` deletion is a compile_error
// mutant (undefined s); the `s += x` deletion is killed (Sum stays 0).
func Sum(xs []int) int {
	s := 0
	for _, x := range xs {
		s += x
	}
	return s
}

// SumEven adds every even number in [0, n). The `i < n` → `<=` mutant
// (LBR and ROR) adds n itself (SumEven(6): 6 vs 12); the `2` → `1` and
// `2` → `3` mutants (ILI) sum the wrong set (15 and 3 vs 6); deleting
// `s += i` (SDL) returns 0; deleting `s := 0` (SDL) is a compile_error.
// The `+= 2` step is deliberate: ILI on `2` never produces a zero step,
// so no mutant turns this loop into an infinite one.
func SumEven(n int) int {
	s := 0
	for i := 0; i < n; i += 2 {
		s += i
	}
	return s
}

// CountClamped counts values in xs outside [lo, hi].
// - `x < lo` → `<=` is killed by x == lo (0 outside, 1 inside the mutant)
// - `x > hi` → `>=` is killed by x == hi
// - `||` → `&&` is killed by [5, 15] (only 15 outside)
// - right-term removal of `x > hi` is killed by [15] (1 → 0)
func CountClamped(xs []int, lo, hi int) int {
	n := 0
	for _, x := range xs {
		if x < lo || x > hi {
			n++
		}
	}
	return n
}
