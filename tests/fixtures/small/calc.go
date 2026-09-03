// Package calc is the small fixture for gopher_mutant.
package calc

func Add(a, b int) int {
	return a + b
}

func Mul(a, b int) int {
	return a * b
}

func IsPositive(x int) bool {
	return x > 0
}

func IsInRange(v, lo, hi int) bool {
	return v >= lo && v <= hi
}

func SumEven(n int) int {
	s := 0
	for i := 0; i < n; i += 2 {
		s += i
	}
	return s
}

func CountClamped(v, lo, hi int) int {
	n := 0
	if v < lo || v > hi {
		n++
	}
	return n
}

func Offset(x int) int {
	y := x
	y += x
	return y
}

func DeferredValue() (value int) {
	defer func() { value = len("x") }()
	return 0
}
