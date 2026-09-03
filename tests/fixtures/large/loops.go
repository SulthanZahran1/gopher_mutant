package large

func LoopTimeout(n int) int {
	total := 0
	for i := 0; i < n; i++ {
		total += i
	}
	return total
}

func LoopSafe(n int) int {
	total := 0
	for i := 0; i < n; i += 2 {
		total += i
	}
	return total
}

func LoopSafeBare(n int) int {
	total := 0
	for i := 0; i < n; i += 2 {
		total += i
	}
	return total
}

func LoopConditionBare(n int) bool {
	return n < 20
}
