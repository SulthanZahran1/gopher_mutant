package large

import "errors"

func Check(v int) (int, error) {
	var err error
	if v < 0 {
		err = errors.New("negative")
	}
	if err != nil {
		return 0, err
	}
	return v, nil
}

func Pair(v int) (int, string) {
	return v, "ok"
}

func ErrorBare(v int) (int, error) {
	return v, nil
}

func DeferResult() (value int) {
	defer func() { value = len("xy") }()
	return 0
}

func RecoverResult() (value int) {
	defer func() {
		if recover() != nil {
			value = len("xyz")
		}
	}()
	panic("boom")
}
