package medium

func TestableCalls(v int) int {
	result := v
	result += 0
	result -= 1
	return result
}

func NotCoveredChange(v int) int {
	result := v
	result += 5
	return result
}

func NotCoveredFlag(v int) bool {
	return v <= 6
}
