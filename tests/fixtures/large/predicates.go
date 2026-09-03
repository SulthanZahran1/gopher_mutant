package large

func InWindow(v int) bool {
	return v > 10 && v < 20
}

func IsEven(v int) bool {
	return v%2 == 0
}

func PredicateBare(v int) bool {
	return v != 3
}

func PredicateUncovered(v int) bool {
	return v == 4
}

func PredicatePair(v int) bool {
	return v >= 2 && v <= 8
}

func PredicatePairBare(v int) bool {
	return v < 50 && v > 5
}
