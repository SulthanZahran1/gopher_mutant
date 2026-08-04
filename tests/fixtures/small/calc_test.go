package calc

import "testing"

// Every assertion below kills at least one mutant; boundary inputs are
// deliberate (see calc.go comments). Nothing here may be weakened or the
// fixture's 100% kill contract breaks.

func TestAdd(t *testing.T) {
	if Add(1, 2) != 3 {
		t.Fatal("Add(1, 2) != 3")
	}
	if Add(0, 0) != 0 {
		t.Fatal("Add(0, 0) != 0")
	}
}

func TestMul(t *testing.T) {
	if Mul(3, 4) != 12 {
		t.Fatal("Mul(3, 4) != 12")
	}
	if Mul(0, 7) != 0 {
		t.Fatal("Mul(0, 7) != 0")
	}
}

func TestIsPositive(t *testing.T) {
	if !IsPositive(5) {
		t.Fatal("IsPositive(5) should be true")
	}
	if IsPositive(0) {
		t.Fatal("IsPositive(0) should be false")
	}
	if IsPositive(-1) {
		t.Fatal("IsPositive(-1) should be false")
	}
}

func TestSumEven(t *testing.T) {
	if SumEven(6) != 6 {
		t.Fatal("SumEven(6) != 6")
	}
	if SumEven(0) != 0 {
		t.Fatal("SumEven(0) != 0")
	}
	if SumEven(1) != 0 {
		t.Fatal("SumEven(1) != 0")
	}
}

func TestIsInRange(t *testing.T) {
	if !IsInRange(5, 0, 10) {
		t.Fatal("IsInRange(5, 0, 10) should be true")
	}
	if !IsInRange(0, 0, 10) {
		t.Fatal("IsInRange(0, 0, 10) should be true (lo edge)")
	}
	if !IsInRange(10, 0, 10) {
		t.Fatal("IsInRange(10, 0, 10) should be true (hi edge)")
	}
	if IsInRange(-1, 0, 10) {
		t.Fatal("IsInRange(-1, 0, 10) should be false")
	}
	if IsInRange(15, 0, 10) {
		t.Fatal("IsInRange(15, 0, 10) should be false")
	}
}

func TestSum(t *testing.T) {
	if Sum([]int{1, 2, 3}) != 6 {
		t.Fatal("Sum([]int{1, 2, 3}) != 6")
	}
	if Sum(nil) != 0 {
		t.Fatal("Sum(nil) != 0")
	}
}

func TestCountClamped(t *testing.T) {
	// Values strictly outside [lo, hi] are counted. Assertions assert ORIGINAL
	// behavior — every mutant deviates on one of these inputs:
	//   x < lo → x <= lo: killed by [0] (mutant counts 0, original doesn't)
	//   x > hi → x >= hi: killed by [10]
	//   || → &&:           killed by [5, 15] (only 15 is outside)
	//   right-term removal: killed by [15] (mutant sees only x < lo)
	//   left-term removal:  killed by [-1] (mutant sees only x > hi)
	if CountClamped([]int{5}, 0, 10) != 0 {
		t.Fatal("CountClamped([5], 0, 10) != 0")
	}
	if CountClamped([]int{0}, 0, 10) != 0 {
		t.Fatal("CountClamped([0], 0, 10) != 0 (0 is inside)")
	}
	if CountClamped([]int{10}, 0, 10) != 0 {
		t.Fatal("CountClamped([10], 0, 10) != 0 (10 is inside)")
	}
	if CountClamped([]int{15}, 0, 10) != 1 {
		t.Fatal("CountClamped([15], 0, 10) != 1")
	}
	if CountClamped([]int{-1}, 0, 10) != 1 {
		t.Fatal("CountClamped([-1], 0, 10) != 1")
	}
	if CountClamped([]int{5, 15}, 0, 10) != 1 {
		t.Fatal("CountClamped([5, 15], 0, 10) != 1")
	}
}
