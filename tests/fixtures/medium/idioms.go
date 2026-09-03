package medium

func DeferredValue() (value int) {
	defer func() { value = 7 }()
	return 0
}

func AsyncValue() int {
	ch := make(chan int)
	go func() {
		ch <- 8
	}()
	return <-ch
}

func ClosedChannelValue() (value int) {
	ch := make(chan int)
	defer func() {
		if recover() != nil {
			value = 9
		}
	}()
	close(ch)
	close(ch)
	return 0
}

func SelectValue(ready <-chan int) int {
	select {
	case value := <-ready:
		return value
	default:
		return 0
	}
}

func AppendValue(values []int) []int {
	values = append(values, 4)
	return values
}

func SliceValue(values []int, index int) int {
	return values[index]
}

func RecoverValue() (value int) {
	defer func() {
		if recover() != nil {
			value = len("xyz")
		}
	}()
	panic("boom")
}
