package large

func SelectReady(ready <-chan int) int {
	select {
	case value := <-ready:
		return value
	default:
		return 0
	}
}

func SelectBare(ready <-chan int) int {
	select {
	case value := <-ready:
		return value
	default:
		return 0
	}
}
