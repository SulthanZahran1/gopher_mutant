package medium

import "errors"

func Add(a, b int) int {
	return a + b
}

func Multiply(a, b int) int {
	return a * b
}

func InRange(v, lo, hi int) bool {
	return v >= lo && v <= hi
}

func LoopTotal(n int) int {
	total := 0
	for i := 0; i < n; i++ {
		total += i
	}
	return total
}

func Outside(v, lo, hi int) int {
	if v < lo || v > hi {
		return 1
	}
	return 0
}

func CheckedValue(v int) (int, error) {
	if v < 0 {
		return 0, errors.New("negative")
	}
	return v, nil
}

func CheckedError(v int) (int, error) {
	var err error
	if v < 0 {
		err = errors.New("negative")
	}
	if err != nil {
		return 0, err
	}
	return v, nil
}

func ReturnPair(v int) (int, string) {
	return v, "ok"
}

func UncheckedArithmetic(v int) int {
	return v + 1
}

func UncheckedRelation(v int) bool {
	return v > 2
}
