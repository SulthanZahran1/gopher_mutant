package large

func AsyncValue() int {
	ch := make(chan int)
	go func() {
		ch <- 8
	}()
	return <-ch
}

func GoroutineBare() int {
	value := 1
	go func() { _ = value }()
	return value
}

func CloseValue() (value int) {
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
