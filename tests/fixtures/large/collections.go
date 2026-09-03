package large

func MapTotal(values map[string]int) int {
	total := 0
	for key, value := range values {
		if key != "" {
			total += value
		}
	}
	return total
}

func RangeTotal(values []int) int {
	total := 0
	for _, value := range values {
		total += value
	}
	return total
}

func RangeBare(values []int) int {
	total := 0
	for _, value := range values {
		total += value
	}
	return total
}

func AppendList(values []int) []int {
	values = append(values, 4)
	return values
}

func SlicePick(values []int, index int) int {
	return values[index]
}
