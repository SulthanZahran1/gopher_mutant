package medium

type Box struct {
	value int
}

func (b Box) Value() int {
	return b.value
}

func UseBox(box Box) int {
	return box.Value()
}

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

func MapSurvivor(values map[string]int) int {
	total := 0
	for _, value := range values {
		total += value
	}
	return total
}

func UncoveredRange(values []int) int {
	total := 0
	for _, value := range values {
		total += value
	}
	return total
}
